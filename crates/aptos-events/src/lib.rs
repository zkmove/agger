use aptos_sdk::bcs;
use aptos_sdk::move_types::identifier::Identifier;
use aptos_sdk::rest_client::aptos_api_types::{
    EntryFunctionId, HexEncodedBytes, IdentifierWrapper, MoveModuleId, VersionedEvent, ViewRequest,
};
use aptos_sdk::rest_client::error::RestError;
pub use aptos_sdk::rest_client::AptosBaseUrl;
use aptos_sdk::rest_client::Client;
pub use aptos_sdk::types::account_address::AccountAddress as AptosAccountAddress;
use aptos_sdk::types::contract_event::{ContractEvent, ContractEventV0, EventWithVersion};
use async_stream::stream;
use futures_core::Stream;
use log::info;
use std::time::Duration;
use tokio::time::sleep;

use agger_contract_types::*;

#[derive(Clone, Debug)]
pub struct AggerQueryManager {
    client: Client,
    param: AggerQueryParam,
}

#[derive(Clone, Debug)]
pub struct AggerQueryParam {
    pub aggger_address: AptosAccountAddress,
}

type AptosResult<T> = Result<T, RestError>;

impl AggerQueryManager {
    pub fn new(aptos_url: AptosBaseUrl, param: AggerQueryParam) -> Self {
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
                    address: self.param.aggger_address.into(),
                    name: IdentifierWrapper(Identifier::new(AGGER_REGISTRY_MODULE_NAME).unwrap()),
                },
                name: IdentifierWrapper(
                    Identifier::new(AGGER_REGISTRY_FUNC_NAME_GET_MODULE).unwrap(),
                ),
            },
            type_arguments: vec![],
            arguments: vec![
                HexEncodedBytes(query.module_address.clone())
                    .json()
                    .unwrap(),
                HexEncodedBytes(query.module_name.clone()).json().unwrap(),
            ],
        };

        let response = self
            .client
            .view(&req, Some(*version))
            .await?
            .into_inner()
            .pop();
        let module_bytes: HexEncodedBytes = response
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_module should return one value");
        // TODO: fetch deps if any
        Ok(vec![module_bytes.0])
    }

    ///return (config, vk,param) for a user query
    pub async fn get_vk_for_query(
        &self,
        module_address: Vec<u8>,
        module_name: Vec<u8>,
        function_index: u16,
        version: u64,
        //UserQuery { query, .. }: &UserQuery,
    ) -> AptosResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let reqs = vec![
            ViewRequest {
                function: EntryFunctionId {
                    module: MoveModuleId {
                        address: self.param.aggger_address.into(),
                        name: IdentifierWrapper(
                            Identifier::new(AGGER_REGISTRY_MODULE_NAME).unwrap(),
                        ),
                    },
                    name: IdentifierWrapper(
                        Identifier::new(AGGER_REGISTRY_FUNC_NAME_GET_CONFIG).unwrap(),
                    ),
                },
                type_arguments: vec![],
                arguments: vec![
                    HexEncodedBytes(module_address.clone()).json().unwrap(),
                    HexEncodedBytes(module_name.clone()).json().unwrap(),
                    serde_json::to_value(function_index).unwrap(),
                ],
            },
            ViewRequest {
                function: EntryFunctionId {
                    module: MoveModuleId {
                        address: self.param.aggger_address.into(),
                        name: IdentifierWrapper(
                            Identifier::new(AGGER_REGISTRY_MODULE_NAME).unwrap(),
                        ),
                    },
                    name: IdentifierWrapper(
                        Identifier::new(AGGER_REGISTRY_FUNC_NAME_GET_VK).unwrap(),
                    ),
                },
                type_arguments: vec![],
                arguments: vec![
                    HexEncodedBytes(module_address.clone()).json().unwrap(),
                    HexEncodedBytes(module_name.clone()).json().unwrap(),
                    serde_json::to_value(function_index).unwrap(),
                ],
            },
            ViewRequest {
                function: EntryFunctionId {
                    module: MoveModuleId {
                        address: self.param.aggger_address.into(),
                        name: IdentifierWrapper(
                            Identifier::new(AGGER_REGISTRY_MODULE_NAME).unwrap(),
                        ),
                    },
                    name: IdentifierWrapper(
                        Identifier::new(AGGER_REGISTRY_FUNC_NAME_GET_PARAM).unwrap(),
                    ),
                },
                type_arguments: vec![],
                arguments: vec![
                    HexEncodedBytes(module_address.clone()).json().unwrap(),
                    HexEncodedBytes(module_name.clone()).json().unwrap(),
                    serde_json::to_value(function_index).unwrap(),
                ],
            },
        ];
        let mut reqs: Vec<_> = reqs
            .iter()
            .map(|req| self.client.view(req, Some(version)))
            .collect();

        let (param, vk, config) = tokio::try_join!(
            reqs.pop().unwrap(),
            reqs.pop().unwrap(),
            reqs.pop().unwrap()
        )?;
        let param: HexEncodedBytes = param
            .into_inner()
            .pop()
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_param return value");
        let vk: HexEncodedBytes = vk
            .into_inner()
            .pop()
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_vk return value");
        let config: HexEncodedBytes = config
            .into_inner()
            .pop()
            .map(serde_json::from_value)
            .transpose()?
            .expect("view get_config return value");

        Ok((config.0, vk.0, param.0))
    }

    pub fn get_query_stream(self) -> impl Stream<Item = AptosResult<UserQuery>> {
        stream! {
            let mut cur = 0;
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
                // if let Some(evt) = event {
                //
                // } else {
                //     sleep(Duration::from_secs(30)).await;
                // }
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
                    self.param.aggger_address,
                    AGGER_QUERY_MODULE_NAME,
                    AGGER_QUERY_QUERIES_STRUCT_NAME
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
                    self.param.aggger_address,
                    AGGER_QUERY_MODULE_NAME,
                    AGGER_QUERY_QUERY_STRUCT_NAME
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
                self.param.aggger_address,
                format!(
                    "{:#x}::{}::{}",
                    self.param.aggger_address,
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
