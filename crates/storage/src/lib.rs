pub use aptos_schemadb as schemadb;
use aptos_schemadb::schema::{KeyCodec, Schema, ValueCodec};
use aptos_schemadb::{define_schema, ColumnFamilyName, DB};

use agger_contract_types::UserQuery;

#[derive(Debug)]
pub struct AggerStore {
    db: DB,
}

impl AggerStore {
    pub fn new(db: DB) -> Self {
        Self { db }
    }
}

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
    const COLUMN_FAMILY_NAME: ColumnFamilyName = "queries";
    type Key = UserQueryKey;

    type Value = UserQueryValue;
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

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}