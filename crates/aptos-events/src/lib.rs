use agger_contract_types::*;
use aptos_sdk::{
    bcs,
    rest_client::{error::RestError, Client},
    types::contract_event::{ContractEvent, EventWithVersion},
};
pub use aptos_sdk::{
    rest_client::AptosBaseUrl, types::account_address::AccountAddress as AptosAccountAddress,
};
use async_stream::stream;
use futures_core::Stream;
use log::info;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct AggerQueries {
    client: Client,
    agger_address: AptosAccountAddress,
}

type AptosResult<T> = Result<T, RestError>;

impl AggerQueries {
    pub fn new(aptos_url: AptosBaseUrl, agger_address: AptosAccountAddress) -> Self {
        Self {
            client: Client::builder(aptos_url).build(),
            agger_address,
        }
    }

    pub fn get_query_stream(self, start: u64) -> impl Stream<Item = AptosResult<UserQuery>> {
        stream! {
            let mut cur = start;
            loop {
                let event = self.get_event(cur).await;
                match event {
                    Ok(Some(evt)) => {
                        let q = self.handle_new_query_event(evt).await;
                        yield q;
                        cur += 1;
                    }
                    Ok(None) => {
                        sleep(Duration::from_secs(30)).await;
                    }
                    Err(e) => {
                        yield Err(e)
                    }
                }
            }
        }
    }

    async fn handle_new_query_event(
        &self,
        EventWithVersion {
            transaction_version,
            event: ContractEvent::V0(event),
        }: EventWithVersion,
    ) -> AptosResult<UserQuery> {
        info!(
            "new query event, key: {}, {}/{}",
            &event.key(),
            &event.type_tag(),
            event.sequence_number()
        );
        let new_query_event: NewQueryEvent = bcs::from_bytes(event.event_data())?;
        let response = self
            .client
            .get_account_resource_at_version_bcs(
                new_query_event.user,
                format!(
                    "{:#x}::{}::{}", // {:#x} to format using hex with '0x' prefix
                    self.agger_address, AGGER_QUERY_MODULE_NAME, AGGER_QUERY_QUERIES_STRUCT_NAME
                )
                .as_str(),
                transaction_version,
            )
            .await?;
        let queries: Queries = response.into_inner();

        let query = self
            .client
            .get_table_item_bcs_at_version::<_, Query>(
                queries.queries.inner.handle,
                "u64",
                format!(
                    "{:#x}::{}::{}",
                    self.agger_address, AGGER_QUERY_MODULE_NAME, AGGER_QUERY_QUERY_STRUCT_NAME
                )
                .as_str(),
                new_query_event.id.to_string(), // to_string is needed, because aptos represent u64/u128 as string.
                transaction_version,
            )
            .await?;
        Ok(UserQuery {
            version: transaction_version,
            sequence_number: event.sequence_number(),
            id: new_query_event.id,
            user: new_query_event.user,
            query: query.into_inner(),
        })
    }

    async fn get_event(&self, at: u64) -> AptosResult<Option<EventWithVersion>> {
        let response = self
            .client
            .get_account_events_bcs(
                self.agger_address,
                format!(
                    "{:#x}::{}::{}",
                    self.agger_address,
                    AGGER_QUERY_MODULE_NAME,
                    AGGER_QUERY_EVENT_HANDLES_STRUCT_NAME
                )
                .as_str(),
                AGGER_QUERY_FIELD_NAME_NEW_EVENT_HANDLE,
                Some(at),
                Some(1),
            )
            .await?;
        let mut events = response.into_inner();
        Ok(events.pop())
    }
}
