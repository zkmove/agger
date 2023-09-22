use movelang::{argument::ScriptArguments, value::TypeTag};
use zkmove_vm_circuit::witness::CircuitConfig as ZkMoveCircuitConfig;

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

impl From<CircuitConfig> for ZkMoveCircuitConfig {
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
        let mut config = ZkMoveCircuitConfig::default()
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
