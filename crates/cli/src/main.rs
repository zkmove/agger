use agger_cli::circuit_config::{parse_entry_function_config, parse_from_move_toml};
use agger_node::vk_generator::{gen_vks, PublishModulesConfig};
use clap::{Parser, Subcommand};
use move_compiler::compiled_unit::CompiledUnit;
use move_package::compilation::compiled_package::OnDiskCompiledPackage;
use move_package::compilation::package_layout::CompiledPackageLayout;
use move_package::source_package::layout::SourcePackageLayout;
use move_package::source_package::manifest_parser::parse_move_manifest_from_file;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(long = "path", short = 'p', value_parser = clap::value_parser ! (PathBuf))]
    package_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    BuildAptosDeploymentFile(BuildAptosDeployment),
}

#[derive(Parser)]
struct BuildAptosDeployment {
    #[arg(short, long = "module")]
    module: String,
    #[arg(long = "agger")]
    agger_address: String,
}

fn main() -> anyhow::Result<()> {
    let cli: Cli = Cli::parse();

    match cli.command {
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
                }
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
            let vks = gen_vks(PublishModulesConfig {
                modules: pkg.all_modules().map(|m| m.unit.serialize(None)).collect(),
                entry_function_config: entry_function_config.into_values().collect(),
            })?;
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
        }
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
