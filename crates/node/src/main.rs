use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use futures_util::{pin_mut, StreamExt, TryFutureExt, TryStreamExt};
use log::error;

use agger_contract_types::UserQuery;
use agger_node::proving::prove;
use agger_node::query_runner::witness;
use agger_node::vk_generator::VerificationParameters;
use agger_storage::schemadb::Options;
use agger_storage::{AggerStore, UserQueryKey, UserQuerySchema, UserQueryValue};
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
    #[arg(long)]
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

                let db = agger_storage::schemadb::DB::open(
                    store_path.as_deref().unwrap_or(Path::new(".")),
                    "agger-db",
                    vec!["queries", "proofs"],
                    &options,
                )?;

                db
            };
            let store = Arc::new(store);

            let event_manager = aptos_events::AggerQueryManager::new(
                parse_aptos_url(&aptos_rpc)?,
                AggerQueryParam {
                    aggger_address: agger_address,
                },
            );
            let new_query_event_stream = event_manager
                .clone()
                .get_query_stream()
                .map_err(|e| anyhow::Error::new(e))
                .and_then(|s| {
                    prepare_prove_data(event_manager.clone(), s.clone())
                        .map_ok(|(ms, vk)| (s, ms, vk))
                })
                .fuse();
            pin_mut!(new_query_event_stream);

            while let Some(s) = new_query_event_stream.next().await {
                println!("{:?}", s);
                match s {
                    Ok((query, modules, vp)) => {
                        store.put::<UserQuerySchema>(
                            &UserQueryKey::from(query.sequence_number),
                            &UserQueryValue::from(query.clone()),
                        )?;

                        let witness = witness(query.clone(), modules, &vp)?;
                        let _proof = prove(query, witness, &vp)?;
                        // submit proof
                    }
                    Err(e) => {
                        error!("get query error. {:?}", e);
                    }
                }
            }

            println!("Hello, world!");
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
