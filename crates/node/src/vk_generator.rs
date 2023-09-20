use std::str::FromStr;

use anyhow::anyhow;
use halo2_proofs::halo2curves::bn256::{Bn256, Fr};
use halo2_proofs::poly::kzg::commitment::ParamsKZG;
use halo2_proofs::SerdeFormat;
use move_core_types::account_address::AccountAddress;
use move_core_types::resolver::ModuleResolver;
use movelang::argument::{IdentStr, Identifier, ScriptArguments};
use movelang::move_binary_format::access::ModuleAccess;
use movelang::move_binary_format::CompiledModule;
use movelang::value::{ModuleId, TypeTag};
use rand::prelude::StdRng;
use rand::SeedableRng;
use zkmove_vm::runtime::Runtime;
use zkmove_vm::state::StateStore;
use zkmove_vm_circuit::circuit::VmCircuit;
use zkmove_vm_circuit::{find_best_k, setup_vm_circuit};

#[derive(Copy, Clone, Debug, Default)]
pub struct CircuitConfig {
    pub max_step_row: Option<usize>,
    pub stack_ops_num: Option<usize>,
    pub locals_ops_num: Option<usize>,
    pub global_ops_num: Option<usize>,
    pub max_frame_index: Option<usize>,
    pub max_locals_size: Option<usize>,
    pub max_stack_size: Option<usize>,
    pub word_size: Option<usize>,
}

impl From<CircuitConfig> for zkmove_vm_circuit::witness::CircuitConfig {
    fn from(
        CircuitConfig {
            max_step_row,
            stack_ops_num,
            locals_ops_num,
            global_ops_num,
            max_frame_index,
            max_locals_size,
            max_stack_size,
            word_size,
        }: CircuitConfig,
    ) -> Self {
        let mut config = zkmove_vm_circuit::witness::CircuitConfig::default()
            .max_step_row(max_step_row)
            .stack_ops_num(stack_ops_num)
            .locals_ops_num(locals_ops_num)
            .global_ops_num(global_ops_num)
            .word_size(word_size);
        if let Some(c) = max_frame_index {
            config = config.max_frame_index(c);
        }
        if let Some(c) = max_locals_size {
            config = config.max_locals_size(c);
        }
        if let Some(c) = max_stack_size {
            config = config.max_stack_size(c);
        }
        config
    }
}

#[derive(Clone, Default, Debug)]
pub struct DemoRunConfig {
    pub args: Option<ScriptArguments>,
    pub ty_args: Option<Vec<TypeTag>>,
}

#[derive(Clone, Debug)]
pub struct EntryFunctionConfig {
    pub entry_module_address: String,
    pub entry_module_name: String,
    pub entry_function: String,
    // TODO: replace it with struct
    pub demo_run_config: DemoRunConfig,
    pub circuit_config: CircuitConfig,
}

pub struct PublishModulesConfig {
    pub modules: Vec<Vec<u8>>,
    pub entry_function_config: Vec<EntryFunctionConfig>,
}

#[derive(Clone, Debug)]
pub struct VerificationParameters {
    pub config: Vec<u8>,
    pub vk: Vec<u8>,
    pub param: Vec<u8>,
}

impl VerificationParameters {
    pub fn new(config: Vec<u8>, vk: Vec<u8>, param: Vec<u8>) -> Self {
        Self { config, vk, param }
    }
}

pub fn gen_vks(
    PublishModulesConfig {
        modules,
        entry_function_config,
    }: PublishModulesConfig,
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

        let params = ParamsKZG::<Bn256>::setup(k, StdRng::from_entropy());
        let (vk, _) = setup_vm_circuit(&vm_circuit, &params)?;

        let mut vk = vk.to_bytes(SerdeFormat::Processed);

        let entry_function_index = compiled_module
            .function_defs()
            .iter()
            .enumerate()
            .find(|(_, fd)| {
                compiled_module.identifier_at(compiled_module.function_handle_at(fd.function).name)
                    == entry_function_name
            })
            .map(|(fdi, _)| fdi)
            .ok_or(anyhow!(
                "expect find index of function {}",
                entry_function_name
            ))? as u16;
        extend_vk_with_func_info(&mut vk, entry_function_index);
        let params = {
            let mut serialzied_param = Vec::new();
            params.write_custom(&mut serialzied_param, SerdeFormat::Processed)?;
            serialzied_param
        };

        vks.push(VerificationParameters {
            config: bcs::to_bytes(&circuit_config)?, // TODO: change to a more common serialization lib.
            vk,
            param: params,
        });
    }
    Ok(vks)
}

fn extend_vk_with_func_info(
    vk: &mut Vec<u8>,
    //_circuit_config: CircuitConfig,
    entry_function_index: u16,
) {
    vk.append(&mut entry_function_index.to_le_bytes().to_vec());
}
