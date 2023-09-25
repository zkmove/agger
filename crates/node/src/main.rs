use agger_node::{open_db, proof_responder::ProofResponder};
use agger_prove_dispatcher::{ProveTask, ProvingTaskDispatcher};
use agger_storage::{AggerStore, UserQueryKey, UserQuerySchema, UserQueryValue};
use aptos_events::{AggerQueries, AptosAccountAddress, AptosBaseUrl};
use clap::Parser;
use futures_util::{pin_mut, StreamExt, TryFutureExt, TryStreamExt};
use log::error;
use query_module_resolver::AggerModuleResolver;
use std::{path::PathBuf, sync::Arc};
use tokio::{select, sync::mpsc};

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
            run_server(
                aptos_rpc,
                agger_address,
                store_path.unwrap_or(PathBuf::from(".")),
            )
            .await?;
            println!("Agger stopped!");
        },
    }

    Ok(())
}

async fn run_server(
    aptos_rpc: String,
    agger_address: AptosAccountAddress,
    store_path: PathBuf,
) -> anyhow::Result<()> {
    let store = open_db(&store_path)?;
    let store = Arc::new(AggerStore::new(store));
    let proof_responder = ProofResponder::new(store.clone());

    let prover_threads = threadpool::Builder::new()
        .thread_name("provers".to_string())
        .build();
    let (task_sender, task_receiver) = mpsc::channel(32);
    let (output_sender, output_receiver) = mpsc::channel(32);

    let provers = ProvingTaskDispatcher::new(prover_threads, task_receiver, output_sender);

    let query_function_resolver =
        AggerModuleResolver::new(parse_aptos_url(&aptos_rpc)?, agger_address);

    let event_manager = AggerQueries::new(parse_aptos_url(&aptos_rpc)?, agger_address);

    //skip proved event
    let query_event_from = store
        .last_proved_event_number()?
        .map(|x| x + 1)
        .unwrap_or(0);
    let new_query_event_stream = event_manager
        .clone()
        .get_query_stream(query_event_from)
        .map_err(anyhow::Error::new)
        .and_then(|s| {
            query_function_resolver
                .clone()
                .get_vk_for_entry_function(
                    s.query.module_address.clone(),
                    s.query.module_name.clone(),
                    s.query.function_name.clone(),
                    s.version,
                )
                .map_ok(|(m, vp)| ProveTask {
                    query: s,
                    modules: vec![m],
                    config: vp.config,
                    vk: vp.vk,
                    param: vp.param,
                })
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
                    Ok(task) => {
                        store.put::<UserQuerySchema>(
                            &UserQueryKey::from(task.query.sequence_number),
                            &UserQueryValue::from(task.query.clone()),
                        )?;

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
    Ok(())
}
