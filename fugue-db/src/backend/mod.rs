use std::path::Path;

use crate::Error;

pub trait Backend {
    fn name() -> &'static str;

    fn is_available() -> bool;
    fn is_preferred_for<P>(path: P) -> bool
    where P: AsRef<Path>;

    fn import_full<P, D, FD, E>(
        &self,
        program: P,
        db_path: D,
        fdb_path: FD,
        overwrite_fdb: bool,
        rebase: Option<u64>,
        rebase_relative: i32,
    ) -> Result<(), E>
    where
        P: AsRef<Path>,
        D: AsRef<Path>,
        FD: AsRef<Path>,
        E: Into<Error>;
}
