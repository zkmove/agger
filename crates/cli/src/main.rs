use agger_cli::circuit_config::{parse_entry_function_config, parse_from_move_toml};
use agger_vk_generation::gen_vks;
use clap::{value_parser, Parser, Subcommand};
use move_compiler::compiled_unit::CompiledUnit;
use move_core_types::{
    account_address::AccountAddress, language_storage::TypeTag, parser::parse_type_tag,
    transaction_argument::TransactionArgument,
};
use move_package::{
    compilation::{compiled_package::OnDiskCompiledPackage, package_layout::CompiledPackageLayout},
    source_package::{layout::SourcePackageLayout, manifest_parser::parse_move_manifest_from_file},
};
use movelang::argument::parse_transaction_argument;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(long = "path", short = 'p', value_parser = value_parser ! (PathBuf))]
    package_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    BuildAptosDeploymentFile(BuildAptosDeployment),
    BuildQuery(BuildQuery),
}

#[derive(Parser)]
struct BuildAptosDeployment {
    #[arg(short, long = "module")]
    module: String,
    #[arg(long = "agger")]
    agger_address: String,
}

#[derive(Parser)]
struct BuildQuery {
    #[arg(long)]
    function_id: String,
    #[arg(long)]
    args: Vec<String>,
    #[arg(long)]
    type_args: Vec<String>,
    #[arg(long = "agger")]
    agger_address: String,
}

fn main() -> anyhow::Result<()> {
    let cli: Cli = Cli::parse();

    match cli.command {
        Commands::BuildQuery(BuildQuery {
            function_id,
            args,
            type_args,
            agger_address,
        }) => {
            // check args and type_args are well formed
            let _ = args
                .iter()
                .map(|a| parse_transaction_argument(a.as_str()))
                .collect::<anyhow::Result<Vec<TransactionArgument>>>()?;
            let _ = type_args
                .iter()
                .map(|a| parse_type_tag(a.as_str()))
                .collect::<anyhow::Result<Vec<TypeTag>>>()?;
            let function_id: Vec<_> = function_id
                .splitn(3, "::")
                .map(ToString::to_string)
                .collect();
            let module_address = AccountAddress::from_hex_literal(&function_id[0])?.to_vec();
            let module_name = function_id[1].to_string();
            let function_name = function_id[2].to_string();

            let aptos_args = vec![
                ArgWithTypeJSON {
                    arg_type: "hex".to_string(),
                    value: serde_json::Value::String(format!("0x{}", hex::encode(module_address))),
                },
                ArgWithTypeJSON {
                    arg_type: "string".to_string(),
                    value: serde_json::Value::String(module_name.clone()),
                },
                ArgWithTypeJSON {
                    arg_type: "string".to_string(),
                    value: serde_json::Value::String(function_name.clone()),
                },
                ArgWithTypeJSON {
                    arg_type: "string".to_string(),
                    value: serde_json::Value::Array(
                        args.into_iter().map(serde_json::Value::String).collect(),
                    ),
                },
                ArgWithTypeJSON {
                    arg_type: "string".to_string(),
                    value: serde_json::Value::Array(
                        type_args
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                },
                ArgWithTypeJSON {
                    arg_type: "u64".to_string(),
                    value: serde_json::Value::Number(100.into()),
                },
            ];
            let json = EntryFunctionArgumentsJSON {
                function_id: format!("{}::query::send_query", agger_address.as_str()),
                type_args: vec![],
                args: aptos_args,
            };

            let output = serde_json::to_string_pretty(&json)?;
            println!("{}", output);
            let project_root = reroot_path(cli.package_path).unwrap();
            let aptos_query_dir = project_root
                .join("queries")
                .join("aptos")
                .join(module_name.as_str());
            std::fs::create_dir_all(aptos_query_dir.as_path())?;
            std::fs::write(
                aptos_query_dir
                    .join(function_name.as_str())
                    .with_extension("json")
                    .as_path(),
                output.as_str(),
            )?;
        },
        Commands::BuildAptosDeploymentFile(c) => {
            let project_root = reroot_path(cli.package_path).unwrap();
            let package_name = parse_move_manifest_from_file(project_root.as_path())?
                .package
                .name
                .to_string();
            let pkg = OnDiskCompiledPackage::from_path(
                &project_root
                    .join("build")
                    .join(package_name)
                    .join(CompiledPackageLayout::BuildInfo.path()),
            )?;

            let pkg = pkg.into_compiled_package()?;

            let target_module = &pkg.get_module_by_name_from_root(&c.module)?.unit;
            let target_module_bytes = target_module.serialize(None);
            let target_module = match target_module {
                CompiledUnit::Module(m) => m,
                CompiledUnit::Script(_) => {
                    unreachable!()
                },
            };
            let target_module_address = target_module.address.as_ref();
            let target_module_name = target_module.name.to_string();
            let circuit_configs = parse_from_move_toml(&std::fs::read_to_string(
                project_root.join(SourcePackageLayout::Manifest.path()),
            )?)?;
            println!("{:#?}", circuit_configs);
            let entry_function_config = parse_entry_function_config(
                circuit_configs,
                &pkg.compiled_package_info.address_alias_instantiation,
            )?;
            let vks = gen_vks(
                pkg.all_modules().map(|m| m.unit.serialize(None)).collect(),
                entry_function_config.into_values().collect(),
            )?;
            let args = vec![
                ArgWithTypeJSON {
                    arg_type: "hex".to_string(),
                    value: serde_json::Value::String(format!(
                        "0x{}",
                        hex::encode(target_module_address)
                    )),
                },
                ArgWithTypeJSON {
                    arg_type: "string".to_string(),
                    value: serde_json::Value::String(target_module_name),
                },
                ArgWithTypeJSON {
                    arg_type: "hex".to_string(),
                    value: serde_json::Value::String(format!(
                        "0x{}",
                        hex::encode(target_module_bytes)
                    )),
                },
                ArgWithTypeJSON {
                    arg_type: "hex".to_string(),
                    value: serde_json::Value::Array(
                        vks.iter()
                            .map(|vk| {
                                serde_json::Value::String(format!("0x{}", hex::encode(&vk.config)))
                            })
                            .collect(),
                    ),
                },
                ArgWithTypeJSON {
                    arg_type: "hex".to_string(),
                    value: serde_json::Value::Array(
                        vks.iter()
                            .map(|vk| {
                                serde_json::Value::String(format!("0x{}", hex::encode(&vk.vk)))
                            })
                            .collect(),
                    ),
                },
                ArgWithTypeJSON {
                    arg_type: "hex".to_string(),
                    value: serde_json::Value::Array(
                        vks.iter()
                            .map(|vk| {
                                serde_json::Value::String(format!("0x{}", hex::encode(&vk.param)))
                            })
                            .collect(),
                    ),
                },
            ];

            let json = EntryFunctionArgumentsJSON {
                function_id: format!("{}::registry::register_module", c.agger_address.as_str()),
                type_args: vec![],
                args,
            };

            let output = serde_json::to_string_pretty(&json)?;
            println!("{}", output);
            let aptos_deployment_dir = project_root.join("deployments").join("aptos");
            std::fs::create_dir_all(aptos_deployment_dir.as_path())?;
            std::fs::write(
                aptos_deployment_dir
                    .join(c.module)
                    .with_extension("json")
                    .as_path(),
                output.as_str(),
            )?;
        },
    }
    Ok(())
}

pub fn reroot_path(path: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let path = path.unwrap_or_else(|| PathBuf::from("."));
    // Always root ourselves to the package root, and then compile relative to that.
    let rooted_path = SourcePackageLayout::try_find_root(&path.canonicalize()?)?;
    std::env::set_current_dir(rooted_path).unwrap();

    Ok(PathBuf::from("."))
}

#[derive(Deserialize, Serialize)]
/// JSON file format for function arguments.
pub struct ArgWithTypeJSON {
    #[serde(rename = "type")]
    pub(crate) arg_type: String,
    pub(crate) value: serde_json::Value,
}

#[derive(Deserialize, Serialize)]
/// JSON file format for entry function arguments.
pub struct EntryFunctionArgumentsJSON {
    pub(crate) function_id: String,
    pub(crate) type_args: Vec<String>,
    pub(crate) args: Vec<ArgWithTypeJSON>,
}
