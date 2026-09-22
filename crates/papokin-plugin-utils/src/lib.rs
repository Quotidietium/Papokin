//! # Pumpkin Plugin Utilities (`papokin-plugin-utils`)
//!
//! 一个面向 Pumpkin 服务器插件的快速、安全且对开发者友好的工具 crate，提供：
//! - **自动元数据缓存**：加载时调用一次 `init(context)`；市场元数据从宿主 WIT 获取并缓存到全局。
//! - **Zero-Argument Updates & Online Licensing**: Check licenses and updates against official Pumpkin Marketplace endpoints without manual arguments.
//! - **Online License Checks**: Verify active licenses with `https://market.pumpkinmc.org/api/v1/rest/check-license`.
//! - **License Checks & Grace Periods**: Local lease management (`license_lease.json`) to prevent outages during marketplace downtime.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use papokin_plugin_api::{Plugin, Context, register_plugin};
//! use papokin_plugin_utils::{init, check_license_online, check_for_updates};
//!
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn new() -> Self { MyPlugin }
//!
//!     fn on_load(&self, context: &Context) -> Result<(), String> {
//!         // 1. Initialize plugin-utils (retrieves verified marketplace metadata from host)
//!         let metadata = papokin_plugin_utils::init(context)
//!             .map_err(|e| format!("Plugin initialization failed: {e}"))?;
//!
//!         // 2. Check license online against marketplace
//!         let license_check = papokin_plugin_utils::check_license_online(None)
//!             .map_err(|e| format!("License check failed: {e}"))?;
//!
//!         if !license_check.valid {
//!             return Err(format!("Invalid license status: {}", license_check.status));
//!         }
//!
//!         // 3. Check for updates (zero arguments required)
//!         if let Ok(update) = papokin_plugin_utils::check_for_updates() {
//!             if update.update_available {
//!                 println!("A new version is available: {:?}", update.latest_version);
//!             }
//!         }
//!
//!         Ok(())
//!     }
//! }
//!
//! register_plugin!(MyPlugin);
//! ```

#![warn(missing_docs)]
#![allow(
    clippy::undocumented_unsafe_blocks,
    clippy::option_if_let_else,
    clippy::collection_is_never_read,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::panic
)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

/// 用于与市场交互的 HTTP 客户端辅助工具。
pub mod http;
/// License checking, validation, and lease management.
pub mod license;
/// Data models for metadata, licenses, and updates.
pub mod models;
/// 针对市场端点的非阻塞更新检查。
pub mod updater;

pub use license::{LicenseChecker, LicenseError};
pub use models::{
    CheckLicenseResponse, CheckUpdateResponse, DEFAULT_MARKETPLACE_URL, LicenseLease,
    LicenseStatus, PapokinMetadata,
};
pub use updater::{UpdateChecker, UpdateError};

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// 已验证插件元数据的全局缓存。
static GLOBAL_METADATA: OnceLock<PapokinMetadata> = OnceLock::new();
/// 插件数据文件夹路径的全局缓存。
static GLOBAL_DATA_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// 使用插件的运行时 `Context` 初始化 `papokin-plugin-utils`。
///
/// 若插件已签名，则检索宿主提供的已验证插件市场元数据，
/// 并将其全局缓存。
///
/// # Errors
///
/// Returns `LicenseError::UnsignedPlugin` if the plugin is not signed or marketplace metadata is missing.
pub fn init(
    context: &papokin_plugin_api::Context,
) -> Result<&'static PapokinMetadata, LicenseError> {
    let data_folder = PathBuf::from(context.get_data_folder());

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(market_meta) = context.get_marketplace_metadata() {
            let meta: PapokinMetadata = market_meta.into();
            return init_with_metadata(meta, data_folder);
        }
        Err(LicenseError::UnsignedPlugin)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = data_folder;
        GLOBAL_METADATA.get().ok_or(LicenseError::NotInitialized)
    }
}

/// 用显式元数据初始化 `papokin-plugin-utils`（适用于测试或自定义初始化）。
///
/// # Errors
///
/// Returns `LicenseError::NotInitialized` if caching fails.
pub fn init_with_metadata(
    metadata: PapokinMetadata,
    data_folder: impl AsRef<Path>,
) -> Result<&'static PapokinMetadata, LicenseError> {
    let folder = data_folder.as_ref().to_path_buf();
    let _ = GLOBAL_DATA_FOLDER.set(folder);
    let _ = GLOBAL_METADATA.set(metadata);

    GLOBAL_METADATA.get().ok_or(LicenseError::NotInitialized)
}

///若已调用 `init`，则返回对全局缓存元数据的引用。
#[must_use]
pub fn get_metadata() -> Option<&'static PapokinMetadata> {
    GLOBAL_METADATA.get()
}

///返回对全局缓存元数据的引用。
///
/// # Errors
///
/// Returns `LicenseError::NotInitialized` if `init(context)` has not been called yet.
pub fn metadata() -> Result<&'static PapokinMetadata, LicenseError> {
    GLOBAL_METADATA.get().ok_or(LicenseError::NotInitialized)
}

///若已初始化，则返回对全局缓存的插件数据文件夹的引用。
#[must_use]
pub fn get_data_folder() -> Option<&'static Path> {
    GLOBAL_DATA_FOLDER.get().map(PathBuf::as_path)
}

/// Checks the license online against the marketplace REST API:
/// `GET /api/v1/rest/check-license?plugin_name={name}&license_key={key}`
///
/// If `license_key_override` is `None`, uses the `license_key` stored in the verified metadata.
///
/// # Errors
///
/// Returns `LicenseError` if querying the marketplace fails or if `init` was not called.
pub fn check_license_online(
    license_key_override: Option<&str>,
) -> Result<CheckLicenseResponse, LicenseError> {
    let meta = metadata()?;
    let folder = get_data_folder().ok_or(LicenseError::NotInitialized)?;
    let checker = LicenseChecker::new(folder);
    checker.check_license_online(meta, license_key_override)
}

/// 使用全局缓存的插件元数据向市场检查更新：
/// `GET /api/v1/rest/check-update?plugin_name={name}&current_version={version}`
///
/// # Errors
///
///若查询市场失败或未调用 `init`，则返回 `UpdateError`。
pub fn check_for_updates() -> Result<CheckUpdateResponse, UpdateError> {
    let meta = metadata().map_err(|_| UpdateError::NotInitialized)?;
    UpdateChecker::new().check_for_updates(&meta.plugin_name, &meta.version, &meta.marketplace_url)
}

/// Evaluates the complete offline license status (metadata + lease cache + grace period)
/// 使用全局缓存的插件数据。
#[must_use]
pub fn evaluate_license(grace_period_days: u32) -> LicenseStatus {
    let Some(folder) = get_data_folder() else {
        return LicenseStatus::Invalid("papokin_plugin_utils has not been initialized".to_string());
    };
    let Some(meta) = get_metadata() else {
        return LicenseStatus::Invalid("papokin_plugin_utils has not been initialized".to_string());
    };
    let checker = LicenseChecker::new(folder);
    checker.evaluate_license(meta, grace_period_days)
}
