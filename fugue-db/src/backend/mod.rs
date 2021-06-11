use std::path::Path;

pub trait Backend {
    type Error: Into<crate::Error>;

    fn name() -> &'static str;

    fn is_available() -> bool;
    fn is_preferred_for<P>(path: P) -> bool
    where P: AsRef<Path>;

    fn import_full<P, D, FD>(
        &self,
        program: P,
        db_path: D,
        fdb_path: FD,
        overwrite_fdb: bool,
        rebase: Option<u64>,
        rebase_relative: i32,
    ) -> Result<(), Self::Error>
    where
        P: AsRef<Path>,
        D: AsRef<Path>,
        FD: AsRef<Path>;
}
