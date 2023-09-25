use agger_contract_types::UserQuery;
use agger_storage::{AggerStore, UserQueryProofSchema};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

/// Responder read proof from store or from message bus, and send it to chain.
pub struct ProofResponder {
    db: Arc<AggerStore>,
}

impl ProofResponder {
    pub fn new(db: Arc<AggerStore>) -> Self {
        Self { db }
    }

    pub async fn start(self, mut receiver: Receiver<(UserQuery, Result<Vec<u8>>)>) -> Result<()> {
        while let Some((query, output)) = receiver.recv().await {
            println!("prove result: {:?}", output);
            self.db
                .put::<UserQueryProofSchema>(&query.sequence_number.into(), &output.into())?;
            // TODO: submit proof
        }
        Ok(())
    }
}
