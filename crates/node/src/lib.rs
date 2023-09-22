use agger_storage::schemadb::{Options, DB};
use std::path::Path;

pub fn open_db(path: impl AsRef<Path>) -> anyhow::Result<DB> {
    // Set the options to create the database if it's missing
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    DB::open(
        path,
        //store_path.as_deref().unwrap_or(Path::new(".")),
        "agger-db",
        vec!["queries", "proofs"],
        &options,
    )
}

pub mod proof_responder;
