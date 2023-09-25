use agger_contract_types::{
    AGGER_REGISTRY_FUNC_NAME_GET_CONFIG, AGGER_REGISTRY_FUNC_NAME_GET_MODULE,
    AGGER_REGISTRY_FUNC_NAME_GET_PARAM, AGGER_REGISTRY_FUNC_NAME_GET_VK,
    AGGER_REGISTRY_MODULE_NAME,
};
use anyhow::anyhow;
use aptos_sdk::{
    move_types::identifier::Identifier as AptosIdentifier,
    rest_client::{
        aptos_api_types::{
            EntryFunctionId, HexEncodedBytes, IdentifierWrapper, MoveModuleId, ViewRequest,
        },
        error::RestError,
        Client,
    },
};
pub use aptos_sdk::{
    rest_client::AptosBaseUrl, types::account_address::AccountAddress as AptosAccountAddress,
};
use futures_util::try_join;
use move_binary_format::CompiledModule;
use move_core_types::identifier::Identifier;
use move_helpers::access_ext::ModuleAccessExt;

//type AptosResult<T> = Result<T, RestError>;

#[derive(Clone, Debug)]
pub struct AggerModuleResolver {
    client: Client,
    agger_address: AptosAccountAddress,
}

impl AggerModuleResolver {
    pub fn new(aptos_url: AptosBaseUrl, agger_address: AptosAccountAddress) -> Self {
        Self {
            client: Client::builder(aptos_url).build(),
            agger_address,
        }
    }

    pub async fn get_module_at_version(
        &self,
        module_address: Vec<u8>,
        module_name: Vec<u8>,
        version: u64,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let req = ViewRequest {
            function: EntryFunctionId {
                module: MoveModuleId {
                    address: self.agger_address.into(),
                    name: IdentifierWrapper(
                        AptosIdentifier::new(AGGER_REGISTRY_MODULE_NAME).unwrap(),
                    ),
                },
                name: IdentifierWrapper(
                    AptosIdentifier::new(AGGER_REGISTRY_FUNC_NAME_GET_MODULE).unwrap(),
                ),
            },
            type_arguments: vec![],
            arguments: vec![
                HexEncodedBytes(module_address).json().unwrap(),
                HexEncodedBytes(module_name).json().unwrap(),
            ],
        };

        let response = self
            .client
            .view(&req, Some(version))
            .await?
            .into_inner()
            .pop();
        let module_byte: Option<HexEncodedBytes> =
            response.map(serde_json::from_value).transpose()?;
        //.expect("view get_module should return one value");

        Ok(module_byte.map(|b| b.0))
    }

    ///return (entry_module, zk_params) for a user query
    pub async fn get_vk_for_entry_function(
        self,
        module_address: Vec<u8>,
        module_name: Vec<u8>,
        function_name: Vec<u8>,
        version: u64,
    ) -> anyhow::Result<(Vec<u8>, EntryFunctionZkParameters)> {
        let target_module_bytes = self
            .get_module_at_version(module_address.clone(), module_name.clone(), version)
            .await?
            .ok_or(anyhow!(
                "module {}::{} not exists",
                &hex::encode(module_address.as_slice()),
                String::from_utf8_lossy(&module_name)
            ))?;

        let function_index = {
            let target_module = CompiledModule::deserialize(&target_module_bytes)
                .map_err(|e| RestError::Unknown(anyhow::Error::new(e)))?;
            let function_name = Identifier::from_utf8(function_name)?;
            let function_def = target_module
                .find_function_def_by_name(function_name.as_ident_str())
                .ok_or(anyhow!(
                    "function {} not found in module {}",
                    function_name.as_str(),
                    target_module.self_id()
                ))?;
            function_def.function.0
        };
        let reqs: Vec<_> = vec![
            (
                AGGER_REGISTRY_FUNC_NAME_GET_CONFIG,
                vec![
                    HexEncodedBytes(module_address.clone()).json().unwrap(),
                    HexEncodedBytes(module_name.clone()).json().unwrap(),
                    serde_json::to_value(function_index).unwrap(),
                ],
            ),
            (
                AGGER_REGISTRY_FUNC_NAME_GET_VK,
                vec![
                    HexEncodedBytes(module_address.clone()).json().unwrap(),
                    HexEncodedBytes(module_name.clone()).json().unwrap(),
                    serde_json::to_value(function_index).unwrap(),
                ],
            ),
            (
                AGGER_REGISTRY_FUNC_NAME_GET_PARAM,
                vec![
                    HexEncodedBytes(module_address.clone()).json().unwrap(),
                    HexEncodedBytes(module_name.clone()).json().unwrap(),
                    serde_json::to_value(function_index).unwrap(),
                ],
            ),
        ]
        .into_iter()
        .map(|(func_name, args)| ViewRequest {
            function: EntryFunctionId {
                module: MoveModuleId {
                    address: self.agger_address.into(),
                    name: IdentifierWrapper(
                        AptosIdentifier::new(AGGER_REGISTRY_MODULE_NAME).unwrap(),
                    ),
                },
                name: IdentifierWrapper(AptosIdentifier::new(func_name).unwrap()),
            },
            type_arguments: vec![],
            arguments: args,
        })
        .collect();

        let mut reqs: Vec<_> = reqs
            .iter()
            .map(|req| self.client.view(req, Some(version)))
            .collect();

        let (param, vk, config) = try_join!(
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
        Ok((
            target_module_bytes,
            EntryFunctionZkParameters {
                config: config.0,
                vk: vk.0,
                param: param.0,
            },
        ))
    }
}

pub struct EntryFunctionZkParameters {
    pub config: Vec<u8>,
    pub vk: Vec<u8>,
    pub param: Vec<u8>,
}
