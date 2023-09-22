use agger_types::{EntryFunctionConfig, VerificationParameters};
use anyhow::anyhow;
use fake_rng::CountingRng;
use halo2_proofs::{
    halo2curves::bn256::{Bn256, Fr},
    poly::kzg::commitment::ParamsKZG,
    SerdeFormat,
};
use move_binary_format::CompiledModule;
use move_core_types::{
    account_address::AccountAddress,
    identifier::{IdentStr, Identifier},
    language_storage::ModuleId,
    resolver::ModuleResolver,
};
use move_helpers::access_ext::ModuleAccessExt;
use std::str::FromStr;
use zkmove_vm::{runtime::Runtime, state::StateStore};
use zkmove_vm_circuit::{circuit::VmCircuit, find_best_k, setup_vm_circuit};

pub fn gen_vks(
    modules: Vec<Vec<u8>>,
    entry_function_config: Vec<EntryFunctionConfig>,
) -> anyhow::Result<Vec<VerificationParameters>> {
    let rt = Runtime::<Fr>::new();
    let mut state = StateStore::new();
    let mut compiled_modules = Vec::default();
    for m in &modules {
        let m = CompiledModule::deserialize(m)?;
        compiled_modules.push(m.clone());
        state.add_module(m);
    }

    let mut vks = Vec::new();
    for EntryFunctionConfig {
        entry_module_address,
        entry_module_name,
        entry_function,
        demo_run_config,
        circuit_config,
    } in entry_function_config
    {
        let entry_module = ModuleId::new(
            AccountAddress::from_str(entry_module_address.as_str())?,
            Identifier::new(entry_module_name.as_str()).unwrap(),
        );

        let compiled_module = CompiledModule::deserialize(
            &state
                .get_module(&entry_module)?
                .ok_or(anyhow!("expect module {} exists", &entry_module))?,
        )?;

        let entry_function_name = IdentStr::new(entry_function.as_str()).unwrap();
        let traces = rt
            .execute_entry_function(
                &entry_module,
                entry_function_name,
                demo_run_config.ty_args.clone().unwrap_or_default(),
                None,
                demo_run_config.args.clone(),
                &mut state,
            )
            .unwrap();

        let witness = rt.process_execution_trace(
            demo_run_config.ty_args.clone().unwrap_or_default(),
            None,
            Some((&entry_module, entry_function_name)),
            compiled_modules.clone(),
            traces,
            circuit_config.into(),
        )?;

        let circuit_config = witness.circuit_config.clone();

        let vm_circuit = VmCircuit { witness };
        let k = find_best_k(&vm_circuit, vec![])?;

        // TODO: change the fake param
        let params = ParamsKZG::<Bn256>::setup(k, CountingRng(42));

        let (vk, _) = setup_vm_circuit(&vm_circuit, &params)?;

        let mut vk = vk.to_bytes(SerdeFormat::Processed);

        let entry_function_index = compiled_module
            .find_function_def_by_name(entry_function_name)
            .ok_or(anyhow!(
                "expect find index of function {}",
                entry_function_name
            ))?
            .function
            .0;

        extend_vk_with_func_info(&mut vk, entry_function_index);
        // let params = {
        //     let mut serialzied_param = Vec::new();
        //     params.write_custom(&mut serialzied_param, SerdeFormat::Processed)?;
        //     serialzied_param
        // };

        vks.push(VerificationParameters {
            config: bcs::to_bytes(&circuit_config)?, // TODO: change to a more common serialization lib.
            vk,
            param: bcs::to_bytes(&k)?,
        });
    }
    Ok(vks)
}

fn extend_vk_with_func_info(vk: &mut Vec<u8>, entry_function_index: u16) {
    vk.append(&mut entry_function_index.to_le_bytes().to_vec());
}
