//! Platform-specific constants and parameters for [ArceOS].
//!
//! Currently supported platform configs can be found in the [configs] directory of
//! the [ArceOS] root.
//!
//! [ArceOS]: https://github.com/arceos-org/arceos
//! [configs]: https://github.com/arceos-org/arceos/tree/main/configs

#![no_std]

#[allow(unused)]
pub use axhal_plat_impl::config::*;

axconfig_gen_macros::include_configs!(env!("AX_CONFIG_PATH"));
