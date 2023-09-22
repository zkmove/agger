use agger_types::{CircuitConfig, DemoRunConfig, EntryFunctionConfig};
use anyhow::Context;
use move_package::resolution::resolution_graph::ResolvedTable;
use movelang::argument::{parse_transaction_argument, parse_type_tags, ScriptArguments};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CircuitTomlConfig {
    pub max_step_row: Option<usize>,
    pub stack_ops_num: Option<usize>,
    pub locals_ops_num: Option<usize>,
    pub global_ops_num: Option<usize>,
    pub max_frame_index: Option<usize>,
    pub max_locals_size: Option<usize>,
    pub max_stack_size: Option<usize>,
    pub word_size: Option<usize>,
    pub args: Option<Vec<String>>,
    pub ty_args: Option<Vec<String>>,
}

pub fn parse_from_move_toml(data: &str) -> anyhow::Result<BTreeMap<String, CircuitTomlConfig>> {
    let mut tval =
        toml::from_str::<toml::Value>(data).context("Unable to parse Move package manifest")?;
    let tval = tval
        .as_table_mut()
        .context("Expected a table at top level")?
        .remove("circuits");
    if let Some(tval) = tval {
        Ok(tval.try_into()?)
    } else {
        Ok(Default::default())
    }
}

pub fn parse_entry_function_config(
    config: BTreeMap<String, CircuitTomlConfig>,
    address_alias_instantiation: &ResolvedTable,
) -> anyhow::Result<BTreeMap<String, EntryFunctionConfig>> {
    let mut result = BTreeMap::new();
    for (entr_function, conf) in config {
        let s: Vec<_> = entr_function
            .splitn(3, "::")
            .map(ToString::to_string)
            .collect();
        let args = if let Some(args) = conf.args {
            Some(ScriptArguments::new(
                args.into_iter()
                    .map(|arg| parse_transaction_argument(&arg))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            ))
        } else {
            None
        };
        let ty_args = if let Some(ty_args) = conf.ty_args {
            Some(
                ty_args
                    .into_iter()
                    .map(|ty_arg| {
                        parse_type_tags(&ty_arg)
                            .and_then(|mut t| t.pop().context("expect parse type ok"))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )
        } else {
            None
        };

        let address = address_alias_instantiation
            .get(&s[0].as_str().into())
            .context(format!("expect address name {} exist", s[0]))?;
        result.insert(entr_function, EntryFunctionConfig {
            entry_module_address: address.to_hex_literal(),
            entry_module_name: s[1].to_string(),
            entry_function: s[2].to_string(),
            demo_run_config: DemoRunConfig { args, ty_args },
            circuit_config: CircuitConfig {
                max_step_row: conf.max_step_row,
                stack_ops_num: conf.stack_ops_num,
                locals_ops_num: conf.locals_ops_num,
                global_ops_num: conf.global_ops_num,
                max_frame_index: conf.max_frame_index,
                max_locals_size: conf.max_locals_size,
                max_stack_size: conf.max_stack_size,
                word_size: conf.word_size,
            },
        });
    }
    Ok(result)
}
