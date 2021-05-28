use std::path::Path;

use super::Error;

pub trait Backend {
    fn is_available() -> bool;
    fn is_preferred_for<P>(path: P) -> bool
    where P: AsRef<Path>;

    fn import_full<P, D, ND>(
        &self,
        program: P,
        db_path: D,
        ndb_path: ND,
        overwrite_fdb: bool,
        rebase: Option<u64>,
        rebase_relative: i32,
    ) -> Result<(), Error>
    where
        P: AsRef<Path>,
        D: AsRef<Path>,
        ND: AsRef<Path>;
}
