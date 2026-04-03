#[cfg(all(feature = "bigint-malachite", feature = "bigint-rug"))]
compile_error!("features `bigint-malachite` and `bigint-rug` are mutually exclusive.");

#[cfg(feature = "bigint-malachite")]
mod core_bigint_malachite;
#[cfg(feature = "bigint-malachite")]
pub use self::core_bigint_malachite::*;

#[cfg(all(feature = "bigint-rug", target_os = "windows"))]
compile_error!("feature `bigint-rug` is not supported on Windows.");

#[cfg(feature = "bigint-rug")]
mod core_bigint_rug;
#[cfg(feature = "bigint-rug")]
pub use core_bigint_rug::*;
