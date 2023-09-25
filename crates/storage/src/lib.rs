use agger_contract_types::UserQuery;
pub use aptos_schemadb as schemadb;
use aptos_schemadb::{
    schema::{KeyCodec, Schema, ValueCodec},
    ColumnFamilyName, ReadOptions, DB,
};
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug)]
pub struct AggerStore {
    db: DB,
}

impl Deref for AggerStore {
    type Target = DB;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl AggerStore {
    pub fn new(db: DB) -> Self {
        Self { db }
    }
    pub fn last_proved_event_number(&self) -> anyhow::Result<Option<u64>> {
        let mut iters = self
            .db
            .iter::<UserQueryProofSchema>(ReadOptions::default())?;
        iters.seek_to_last();
        let value = iters.next().transpose()?.map(|(k, _)| k.sequence_number);
        Ok(value)
    }
    pub fn last_seen_event(&self) -> anyhow::Result<Option<UserQuery>> {
        let mut iters = self.db.iter::<UserQuerySchema>(ReadOptions::default())?;
        iters.seek_to_last();
        let value = iters.next().transpose()?.map(|(_k, v)| v.query);
        Ok(value)
    }
}

pub const QUERY_COLUMN_FAMILY_NAME: &str = "queries";
pub const PROOF_COLUMN_FAMILY_NAME: &str = "proofs";

#[derive(Clone, Debug)]
pub struct UserQueryKey {
    sequence_number: u64,
}

impl From<u64> for UserQueryKey {
    fn from(value: u64) -> Self {
        Self {
            sequence_number: value,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserQueryValue {
    query: UserQuery,
}

impl From<UserQuery> for UserQueryValue {
    fn from(value: UserQuery) -> Self {
        Self { query: value }
    }
}

#[derive(Debug)]
pub struct UserQuerySchema;

impl Schema for UserQuerySchema {
    type Key = UserQueryKey;
    type Value = UserQueryValue;

    const COLUMN_FAMILY_NAME: ColumnFamilyName = QUERY_COLUMN_FAMILY_NAME;
}

impl KeyCodec<UserQuerySchema> for UserQueryKey {
    fn encode_key(&self) -> anyhow::Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self.sequence_number)?)
    }

    fn decode_key(data: &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            sequence_number: bcs::from_bytes(data)?,
        })
    }
}

impl ValueCodec<UserQuerySchema> for UserQueryValue {
    fn encode_value(&self) -> anyhow::Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self.query)?)
    }

    fn decode_value(data: &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            query: bcs::from_bytes(data)?,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserQueryProvingResult {
    success: bool,
    result: Vec<u8>,
    /// whether the proof is submitted to onchain.
    submitted: bool,
}

impl From<anyhow::Result<Vec<u8>>> for UserQueryProvingResult {
    fn from(value: anyhow::Result<Vec<u8>>) -> Self {
        match value {
            Err(e) => Self {
                success: false,
                result: e.root_cause().to_string().into_bytes(),
                submitted: false,
            },
            Ok(v) => Self {
                success: true,
                result: v,
                submitted: false,
            },
        }
    }
}

#[derive(Debug)]
pub struct UserQueryProofSchema;

impl Schema for UserQueryProofSchema {
    type Key = UserQueryKey;
    type Value = UserQueryProvingResult;

    const COLUMN_FAMILY_NAME: ColumnFamilyName = PROOF_COLUMN_FAMILY_NAME;
}

impl KeyCodec<UserQueryProofSchema> for UserQueryKey {
    fn encode_key(&self) -> anyhow::Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self.sequence_number)?)
    }

    fn decode_key(data: &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            sequence_number: bcs::from_bytes(data)?,
        })
    }
}

impl ValueCodec<UserQueryProofSchema> for UserQueryProvingResult {
    fn encode_value(&self) -> anyhow::Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self)?)
    }

    fn decode_value(data: &[u8]) -> anyhow::Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}
