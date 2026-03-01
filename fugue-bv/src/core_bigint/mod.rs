#[cfg(all(feature = "bigint-malachite", feature = "bigint-rug"))]
compile_error!("features `bigint-malachite` and `bigint-rug` are mutually exclusive.");

#[cfg(any(
    feature = "bigint-malachite",
    all(feature = "bigint", not(feature = "bigint-rug"), target_os = "windows")
))]
mod core_bigint_malachite;
#[cfg(any(
    feature = "bigint-malachite",
    all(feature = "bigint", not(feature = "bigint-rug"), target_os = "windows")
))]
pub use self::core_bigint_malachite::*;

#[cfg(any(
    feature = "bigint-rug",
    all(feature = "bigint", not(feature = "bigint-malachite"), not(target_os = "windows"))
))]
mod core_bigint_rug;
#[cfg(any(
    feature = "bigint-rug",
    all(feature = "bigint", not(feature = "bigint-malachite"), not(target_os = "windows"))
))]
pub use core_bigint_rug::*;
