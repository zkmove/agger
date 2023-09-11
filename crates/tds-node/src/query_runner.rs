use crate::vk_generator::CircuitConfig;
use anyhow::anyhow;
use aptos_events::UserQuery;
use halo2_proofs::halo2curves::pasta::Fp;
use movelang::argument::{parse_transaction_argument, parse_type_tags, ScriptArguments};
use movelang::move_binary_format::file_format::empty_script;
use movelang::move_binary_format::CompiledModule;
use zkmove_vm::runtime::Runtime;
use zkmove_vm::state::StateStore;
use zkmove_vm_circuit::witness::Witness;

pub fn witness(
    query: UserQuery,
    modules: Vec<Vec<u8>>,
    circuit_config: CircuitConfig,
) -> anyhow::Result<Witness<Fp>> {
    let mut state = StateStore::new();
    let mut compiled_modules = Vec::default();
    for m in &modules {
        let m = CompiledModule::deserialize(m)?;
        compiled_modules.push(m.clone());
        state.add_module(m);
        // todo: replace it with execute entry_function
    }
    let rt = Runtime::<Fp>::new();
    let ty_args = query
        .query
        .ty_args
        .into_iter()
        .map(|t| {
            let s = String::from_utf8(t)?;
            let mut ts = parse_type_tags(s.as_str())?;
            ts.pop().ok_or_else(|| anyhow!("parse type arg failure"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let args = query
        .query
        .args
        .into_iter()
        .map(|arg| {
            let s = String::from_utf8(arg)?;
            let ta = parse_transaction_argument(s.as_str())?;
            Ok(ta)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let witness = rt
        .execute_script(
            empty_script(),
            compiled_modules.clone(),
            ty_args,
            None,
            if args.is_empty() {
                None
            } else {
                Some(ScriptArguments::new(args))
            },
            &mut state,
            circuit_config.into(),
        )
        .unwrap();
    Ok(witness)
}
