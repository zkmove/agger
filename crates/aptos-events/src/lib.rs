use aptos_sdk::rest_client::aptos_api_types::{MoveStructTag, VersionedEvent};
use aptos_sdk::rest_client::error::RestError;
pub use aptos_sdk::rest_client::AptosBaseUrl;
use aptos_sdk::rest_client::Client;
use aptos_sdk::types::account_address::AccountAddress;
use aptos_sdk::types::state_store::table::TableInfo;
use async_stream::try_stream;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct TdsQueryMananger {
    client: Client,
    param: TdsQueryParam,
}

#[derive(Clone, Debug)]
pub struct TdsQueryParam {
    tds_address: AccountAddress,
    event_handle: String,
    field_name: String,
    tds_qyery_module_name: String,
    tds_queries_struct_name: String,
}
const TDS_QUERY_MODULE_NAME: &str = "query";
const TDS_QUERY_QUERY_STRUCT_NAME: &str = "Query";
const TDS_QUERY_QUERIES_STRUCT_NAME: &str = "Queries";
const TDS_QUERY_EVENT_HANDLES_STRUCT_NAME: &str = "EventHandles";
const TDS_QUERY_FIELD_NAME_NEW_EVENT_HANDLE: &str = "new_event_handle";

type AptosResult<T> = Result<T, RestError>;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct NewQueryEvent {
    user: AccountAddress,
    id: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Query {
    module_address: AccountAddress,
    module_name: Vec<u8>,
    function_index: u16,
    deadline: u64,
    success: Option<bool>,
    result: Option<Vec<u8>>,
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
    id: u64,
    user: AccountAddress,
    query: Query,
    /// version at the query is triggered
    version: u64,
}
impl TdsQueryMananger {
    pub fn new(aptos_url: AptosBaseUrl, param: TdsQueryParam) -> Self {
        Self {
            client: Client::builder(aptos_url).build(),
            param,
        }
    }
    pub fn get_event_stream(self) -> impl Stream<Item = AptosResult<UserQuery>> {
        try_stream! {
            let mut cur = 0;
            loop {
                let event = self.get_event(cur).await?;
                if let Some(evt) = event {
                    let q = self.handle_new_query_event(evt).await?;
                    yield q;
                    cur = cur + 1;
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
