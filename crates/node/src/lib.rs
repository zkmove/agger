use agger_storage::{
    schemadb::{Options, DB},
    PROOF_COLUMN_FAMILY_NAME, QUERY_COLUMN_FAMILY_NAME,
};
use std::path::Path;

pub mod proof_responder;

pub fn open_db(path: impl AsRef<Path>) -> anyhow::Result<DB> {
    // Set the options to create the database if it's missing
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    DB::open(
        path,
        //store_path.as_deref().unwrap_or(Path::new(".")),
        "agger-db",
        vec![QUERY_COLUMN_FAMILY_NAME, PROOF_COLUMN_FAMILY_NAME],
        &options,
    )
}
