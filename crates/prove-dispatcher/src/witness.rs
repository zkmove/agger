use agger_contract_types::UserQuery;
use anyhow::anyhow;
use halo2_proofs::halo2curves::bn256::Fr;
use move_binary_format::CompiledModule;
use move_core_types::{
    identifier::Identifier,
    language_storage::ModuleId,
    parser::{parse_transaction_argument, parse_type_tags},
};
use movelang::argument::ScriptArguments;
use zkmove_vm::{runtime::Runtime, state::StateStore};
use zkmove_vm_circuit::witness::Witness;

pub fn witness(
    query: UserQuery,
    modules: Vec<Vec<u8>>,
    config: Vec<u8>,
) -> anyhow::Result<Witness<Fr>> {
    let mut state = StateStore::new();
    let mut compiled_modules = Vec::default();
    for m in &modules {
        let m = CompiledModule::deserialize(m)?;
        compiled_modules.push(m.clone());
        state.add_module(m);
        // todo: replace it with execute entry_function
    }
    let rt = Runtime::<Fr>::new();
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
    let entry_module_address =
        move_core_types::account_address::AccountAddress::from_bytes(&query.query.module_address)?;
    let entry_module_name = Identifier::from_utf8(query.query.module_name.clone())?;
    let entry_function_name = Identifier::from_utf8(query.query.function_name)?;
    let entry_module_id = ModuleId::new(entry_module_address, entry_module_name);
    let traces = rt
        .execute_entry_function(
            &entry_module_id,
            &entry_function_name,
            ty_args.clone(),
            None,
            if args.is_empty() {
                None
            } else {
                Some(ScriptArguments::new(args))
            },
            &mut state,
        )
        .unwrap();

    let witness = rt.process_execution_trace(
        ty_args,
        None,
        Some((&entry_module_id, &entry_function_name)),
        compiled_modules.clone(),
        traces,
        bcs::from_bytes(config.as_slice())?,
    )?;
    Ok(witness)
}
