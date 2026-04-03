#[cfg(all(
    feature = "fp",
    any(
        feature = "bigint-malachite",
        feature = "fixed-u64",
        feature = "fixed-u128",
    )
))]
compile_error!(
    "the `fp` feature is only compatible with the default `bigint`/`bigint-rug` feature"
);

pub use fugue_bv as bv;
#[cfg(feature = "db")]
pub use fugue_db as db;
#[cfg(feature = "fp")]
pub use fugue_fp as fp;
pub use fugue_ir as ir;

pub use fugue_arch as arch;
pub use fugue_bytes as bytes;
pub use fugue_fspec as fspec;
pub use fugue_sleigh as sleigh;
