use aptos_move_core_types::account_address::AccountAddress as AptosAccountAddress;
use serde::{Deserialize, Serialize};

pub const AGGER_REGISTRY_MODULE_NAME: &str = "registry";
pub const AGGER_QUERY_MODULE_NAME: &str = "query";
pub const AGGER_QUERY_QUERY_STRUCT_NAME: &str = "Query";
pub const AGGER_QUERY_QUERIES_STRUCT_NAME: &str = "Queries";
pub const AGGER_QUERY_EVENT_HANDLES_STRUCT_NAME: &str = "EventHandles";
pub const AGGER_QUERY_FIELD_NAME_NEW_EVENT_HANDLE: &str = "new_event_handle";
pub const AGGER_REGISTRY_FUNC_NAME_GET_MODULE: &str = "get_module";
pub const AGGER_REGISTRY_FUNC_NAME_GET_VK: &str = "get_vk";
pub const AGGER_REGISTRY_FUNC_NAME_GET_PARAM: &str = "get_param";
pub const AGGER_REGISTRY_FUNC_NAME_GET_CONFIG: &str = "get_config";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewQueryEvent {
    pub user: AptosAccountAddress,
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Query {
    pub module_address: Vec<u8>,
    pub module_name: Vec<u8>,
    pub function_name: Vec<u8>,
    pub deadline: u64,
    pub args: Vec<Vec<u8>>,
    pub ty_args: Vec<Vec<u8>>,
    pub success: Option<bool>,
    pub result: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Queries {
    pub query_counter: u64,
    pub queries: TableWithLength,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableWithLength {
    pub inner: Table,
    pub length: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Table {
    pub handle: AptosAccountAddress,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserQuery {
    /// version at the query is triggered
    pub version: u64,
    pub sequence_number: u64,
    pub user: AptosAccountAddress,
    pub id: u64,
    pub query: Query,
}
