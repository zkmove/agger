use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use futures_util::{pin_mut, StreamExt, TryFutureExt, TryStreamExt};
use log::error;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

use agger_contract_types::UserQuery;
use agger_node::proving::{ProveTask, ProvingTaskDispatcher};
use agger_node::vk_generator::VerificationParameters;
use agger_storage::schemadb::{Options, DB};
use agger_storage::{UserQueryKey, UserQueryProofSchema, UserQuerySchema, UserQueryValue};
use aptos_events::{AggerQueryManager, AggerQueryParam, AptosAccountAddress, AptosBaseUrl};

#[derive(Parser, Debug)]
enum Cli {
    StartServer(StartServer),
}

#[derive(Parser, Clone, Debug)]
struct StartServer {
    /// aptos rpc, or use devnet,testnet,mainnet
    #[arg(long)]
    aptos_rpc: String,
    /// agger contracts address
    #[arg(long)]
    agger_address: AptosAccountAddress,
    /// storage path
    #[arg(long, default_value = "aggerdb")]
    store_path: Option<PathBuf>,
}

fn parse_aptos_url(rpc: &str) -> anyhow::Result<AptosBaseUrl> {
    let url = match rpc.trim().to_lowercase().as_str() {
        "mainnet" => AptosBaseUrl::Mainnet,
        "devnet" => AptosBaseUrl::Devnet,
        "testnet" => AptosBaseUrl::Testnet,
        _ => AptosBaseUrl::Custom(rpc.trim().parse()?),
    };
    Ok(url)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::try_init()?;
    let cli: Cli = Cli::parse();
    println!("cmd:{:?}", &cli);
    match cli {
        Cli::StartServer(StartServer {
            aptos_rpc,
            agger_address,
            store_path,
        }) => {
            let store = {
                // Set the options to create the database if it's missing
                let mut options = Options::default();
                options.create_if_missing(true);
                options.create_missing_column_families(true);

                agger_storage::schemadb::DB::open(
                    store_path.as_deref().unwrap_or(Path::new(".")),
                    "agger-db",
                    vec!["queries", "proofs"],
                    &options,
                )?
            };
            let store = Arc::new(store);
            let proof_responder = ProofResponder { db: store.clone() };

            let prover_threads = threadpool::Builder::new()
                .thread_name("provers".to_string())
                .build();
            let (task_sender, task_receiver) = mpsc::channel(32);
            let (output_sender, output_receiver) = mpsc::channel(32);
            let provers = ProvingTaskDispatcher::new(prover_threads, task_receiver, output_sender);

            let event_manager = AggerQueryManager::new(
                parse_aptos_url(&aptos_rpc)?,
                AggerQueryParam {
                    aggger_address: agger_address,
                },
            );
            let new_query_event_stream = event_manager
                .clone()
                .get_query_stream()
                .map_err(anyhow::Error::new)
                .and_then(|s| {
                    prepare_prove_data(event_manager.clone(), s.clone())
                        .map_ok(|(ms, vk)| (s, ms, vk))
                })
                .fuse();
            pin_mut!(new_query_event_stream);

            let mut dispatch_task_handle = tokio::spawn(provers.run());
            let mut output_handle = tokio::spawn(proof_responder.start(output_receiver));
            loop {
                select! {
                    _output_task_result = &mut output_handle => {
                        // when output handle is gone, then output receiver is gone.
                        // then dispatcher will go down.
                    }
                    _dispatch_task_result = &mut dispatch_task_handle => {
                        // when dispatcher is gone, then task_sender cannot send any task,
                        // it will go down automatically.
                    }
                    Some(s) = new_query_event_stream.next() => {
                        match s {
                            Ok((query, modules, vp)) => {
                                store.put::<UserQuerySchema>(
                                    &UserQueryKey::from(query.sequence_number),
                                    &UserQueryValue::from(query.clone()),
                                )?;
                                let task  = ProveTask {query: query.clone(), modules, vp};
                                if let Err(_err)  = task_sender.send(task).await {
                                    error!("prover dispatcher is down");
                                    break;
                                }

                            }
                            Err(e) => {
                                error!("get query error. {:?}", e);
                            }
                        }
                    }
                    else => {
                        // no query events anymore
                        break;
                    }
                }
            }

            println!("Agger stopped!");
        }
    }

    Ok(())
}

async fn prepare_prove_data(
    event_manager: AggerQueryManager,
    query: UserQuery,
) -> anyhow::Result<(Vec<Vec<u8>>, VerificationParameters)> {
    let modules = event_manager.prepare_modules(&query).await?;
    let (config, vk, param) = event_manager.get_vk_for_query(&query).await?;
    let vp = VerificationParameters::new(config, vk, param);
    Ok((modules, vp))
}

struct ProofResponder {
    db: Arc<DB>,
}

impl ProofResponder {
    async fn start(
        self,
        mut receiver: Receiver<(UserQuery, anyhow::Result<Vec<u8>>)>,
    ) -> anyhow::Result<()> {
        while let Some((query, output)) = receiver.recv().await {
            println!("prove result: {:?}", output);
            self.db
                .put::<UserQueryProofSchema>(&query.sequence_number.into(), &output.into())?;
            // TODO: submit proof
        }
        Ok(())
    }
}
