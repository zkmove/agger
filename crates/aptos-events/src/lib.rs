use std::time::Duration;

use aptos_sdk::move_types::identifier::Identifier;
use aptos_sdk::rest_client::aptos_api_types::{
    EntryFunctionId, IdentifierWrapper, MoveModuleId, VersionedEvent, ViewRequest,
};
use aptos_sdk::rest_client::error::RestError;
pub use aptos_sdk::rest_client::AptosBaseUrl;
use aptos_sdk::rest_client::Client;
use aptos_sdk::types::account_address::AccountAddress;
use async_stream::try_stream;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct TdsQueryManager {
    client: Client,
    param: TdsQueryParam,
}

#[derive(Clone, Debug)]
pub struct TdsQueryParam {
    tds_address: AccountAddress,
}

const TDS_QUERY_MODULE_NAME: &str = "query";
const TDS_QUERY_QUERY_STRUCT_NAME: &str = "Query";
const TDS_QUERY_QUERIES_STRUCT_NAME: &str = "Queries";
const TDS_QUERY_EVENT_HANDLES_STRUCT_NAME: &str = "EventHandles";
const TDS_QUERY_FIELD_NAME_NEW_EVENT_HANDLE: &str = "new_event_handle";
const TDS_QUERY_FUNC_NAME_GET_MODULE: &str = "get_module";
const TDS_QUERY_FUNC_NAME_GET_VK: &str = "get_vk";
const TDS_QUERY_FUNC_NAME_GET_PARAM: &str = "get_param";
const TDS_QUERY_FUNC_NAME_GET_CONFIG: &str = "get_config";

type AptosResult<T> = Result<T, RestError>;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct NewQueryEvent {
    user: AccountAddress,
    id: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Query {
    pub module_address: Vec<u8>,
    pub module_name: Vec<u8>,
    pub function_index: u16,
    pub deadline: u64,
    pub args: Vec<Vec<u8>>,
    pub ty_args: Vec<Vec<u8>>,
    pub success: Option<bool>,
    pub result: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Queries {
    query_counter: u64,
    queries: TableWithLength,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TableWithLength {
    inner: Table,
    length: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Table {
    handle: AccountAddress,
}

#[derive(Clone, Debug)]
pub struct UserQuery {
    pub id: u64,
    pub user: AccountAddress,
    pub query: Query,
    /// version at the query is triggered
    pub version: u64,
}

impl TdsQueryManager {
    pub fn new(aptos_url: AptosBaseUrl, param: TdsQueryParam) -> Self {
        Self {
            client: Client::builder(aptos_url).build(),
            param,
        }
    }
    pub async fn prepare_modules(
        &self,
        UserQuery { query, version, .. }: &UserQuery,
    ) -> AptosResult<Vec<Vec<u8>>> {
        let req = ViewRequest {
            function: EntryFunctionId {
                module: MoveModuleId {
                    address: self.param.tds_address.into(),
                    name: IdentifierWrapper(Identifier::new(TDS_QUERY_MODULE_NAME).unwrap()),
                },
                name: IdentifierWrapper(Identifier::new(TDS_QUERY_FUNC_NAME_GET_MODULE).unwrap()),
            },
            type_arguments: vec![],
            arguments: vec![
                serde_json::to_value(query.module_address.clone()).unwrap(),
                serde_json::to_value(query.module_name.clone()).unwrap(),
            ],
        };
        let response = self
            .client
            .view(&req, Some(*version))
            .await?
            .into_inner()
            .pop();
        let module_bytes: Vec<_> = response
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_module should return one value");
        // TODO: fetch deps if any
        Ok(vec![module_bytes])
    }
    pub async fn get_vk_for_query(
        &self,
        UserQuery { query, version, .. }: &UserQuery,
    ) -> AptosResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let reqs = vec![
            ViewRequest {
                function: EntryFunctionId {
                    module: MoveModuleId {
                        address: self.param.tds_address.into(),
                        name: IdentifierWrapper(Identifier::new(TDS_QUERY_MODULE_NAME).unwrap()),
                    },
                    name: IdentifierWrapper(
                        Identifier::new(TDS_QUERY_FUNC_NAME_GET_CONFIG).unwrap(),
                    ),
                },
                type_arguments: vec![],
                arguments: vec![
                    serde_json::to_value(query.module_address.clone()).unwrap(),
                    serde_json::to_value(query.module_name.clone()).unwrap(),
                    serde_json::to_value(query.function_index).unwrap(),
                ],
            },
            ViewRequest {
                function: EntryFunctionId {
                    module: MoveModuleId {
                        address: self.param.tds_address.into(),
                        name: IdentifierWrapper(Identifier::new(TDS_QUERY_MODULE_NAME).unwrap()),
                    },
                    name: IdentifierWrapper(Identifier::new(TDS_QUERY_FUNC_NAME_GET_VK).unwrap()),
                },
                type_arguments: vec![],
                arguments: vec![
                    serde_json::to_value(query.module_address.clone()).unwrap(),
                    serde_json::to_value(query.module_name.clone()).unwrap(),
                    serde_json::to_value(query.function_index).unwrap(),
                ],
            },
            ViewRequest {
                function: EntryFunctionId {
                    module: MoveModuleId {
                        address: self.param.tds_address.into(),
                        name: IdentifierWrapper(Identifier::new(TDS_QUERY_MODULE_NAME).unwrap()),
                    },
                    name: IdentifierWrapper(
                        Identifier::new(TDS_QUERY_FUNC_NAME_GET_PARAM).unwrap(),
                    ),
                },
                type_arguments: vec![],
                arguments: vec![
                    serde_json::to_value(query.module_address.clone()).unwrap(),
                    serde_json::to_value(query.module_name.clone()).unwrap(),
                    serde_json::to_value(query.function_index).unwrap(),
                ],
            },
        ];
        let mut reqs: Vec<_> = reqs
            .iter()
            .map(|req| self.client.view(req, Some(*version)))
            .collect();

        let (param, vk, config) = tokio::try_join!(
            reqs.pop().unwrap(),
            reqs.pop().unwrap(),
            reqs.pop().unwrap()
        )?;
        let param: Vec<u8> = param
            .into_inner()
            .pop()
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_param return value");
        let vk: Vec<u8> = vk
            .into_inner()
            .pop()
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_vk return value");
        let config: Vec<u8> = config
            .into_inner()
            .pop()
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_config return value");

        Ok((config, vk, param))
    }

    pub fn get_query_stream(self) -> impl Stream<Item = AptosResult<UserQuery>> {
        try_stream! {
            let mut cur = 0;
            loop {
                let event = self.get_event(cur).await?;
                if let Some(evt) = event {
                    let q = self.handle_new_query_event(evt).await?;
                    yield q;
                    cur += 1;
                } else {
                    sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }
    async fn handle_new_query_event(&self, event: VersionedEvent) -> AptosResult<UserQuery> {
        let new_query_event: NewQueryEvent = serde_json::from_value(event.data)?;
        let version = event.version.0;
        let response = self
            .client
            .get_account_resource_at_version_bcs(
                new_query_event.user,
                format!(
                    "{}/{}/{}",
                    self.param.tds_address, TDS_QUERY_MODULE_NAME, TDS_QUERY_QUERIES_STRUCT_NAME
                )
                .as_str(),
                version,
            )
            .await?;
        let queries: Queries = response.into_inner();

        let query = self
            .client
            .get_table_item_bcs_at_version::<_, Query>(
                queries.queries.inner.handle,
                "u64",
                format!(
                    "{}/{}/{}",
                    self.param.tds_address, TDS_QUERY_MODULE_NAME, TDS_QUERY_QUERY_STRUCT_NAME
                )
                .as_str(),
                new_query_event.id,
                version,
            )
            .await?;
        Ok(UserQuery {
            id: new_query_event.id,
            user: new_query_event.user,
            query: query.into_inner(),
            version,
        })
    }
    async fn get_event(&self, at: u64) -> AptosResult<Option<VersionedEvent>> {
        let response = self
            .client
            .get_account_events(
                self.param.tds_address,
                format!(
                    "{}/{}/{}",
                    self.param.tds_address,
                    TDS_QUERY_MODULE_NAME,
                    TDS_QUERY_EVENT_HANDLES_STRUCT_NAME
                )
                .as_str(),
                TDS_QUERY_FIELD_NAME_NEW_EVENT_HANDLE,
                Some(at),
                Some(1),
            )
            .await?;
        let mut events = response.into_inner();
        Ok(events.pop())
    }
}
