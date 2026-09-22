//! Pumpkin 插件授权、元数据与市场端点的数据模型。

use serde::{Deserialize, Serialize};

/// 默认的 Pumpkin Marketplace URL。
pub const DEFAULT_MARKETPLACE_URL: &str = "https://market.pumpkinmc.org";

/// 由市场或开发者内嵌到 Pumpkin WASM 插件中的元数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PapokinMetadata {
    /// 此插件注册所在的市场基础 URL。
    pub marketplace_url: String,
    /// 市场中插件的唯一 ID。
    pub plugin_id: i64,
    /// 插件的规范名称。
    pub plugin_name: String,
    /// 插件的语义化版本（semver）字符串。
    pub version: String,
    /// 开发者 ID。
    pub dev_id: i64,
    /// 开发者显示名称或用户名。
    pub dev_name: String,
    /// 这是否为付费市场插件。
    pub is_paid: bool,
    /// The buyer / licensee user ID (0 for free/open-source).
    pub user_id: i64,
    /// Unique license key issued to the buyer, if paid.
    pub license_key: Option<String>,
    /// ISO-8601 timestamp of when this binary/license was issued.
    pub issued_at: String,
}

impl From<papokin_plugin_api::MarketplaceMetadata> for PapokinMetadata {
    fn from(m: papokin_plugin_api::MarketplaceMetadata) -> Self {
        Self {
            marketplace_url: m.marketplace_url,
            plugin_id: m.plugin_id,
            plugin_name: m.plugin_name,
            version: m.version,
            dev_id: m.dev_id,
            dev_name: m.dev_name,
            is_paid: m.is_paid,
            user_id: m.user_id,
            license_key: m.license_key,
            issued_at: m.issued_at,
        }
    }
}

/// Result of evaluating a plugin's license.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseStatus {
    /// The license is completely valid.
    Valid(PapokinMetadata),
    /// 在持有有效缓存租约的离线宽限期内运行。
    GracePeriod {
        /// 元数据。
        metadata: PapokinMetadata,
        /// 宽限期的剩余天数。
        days_remaining: u32,
        /// 处于宽限期运行的原因（例如市场不可达）。
        reason: String,
    },
    /// The license is invalid, expired, revoked, or tampered.
    Invalid(String),
    /// 插件二进制文件未附带签名或元数据。
    Unsigned,
}

/// Cached license verification lease stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseLease {
    /// 插件名称。
    pub plugin_name: String,
    /// License key verified.
    pub license_key: Option<String>,
    /// 市场返回的状态字符串（"valid"、"invalid"、"revoked"）。
    pub status: String,
    /// 此租约在线验证时的 Unix 时间戳（秒）。
    pub last_verified_timestamp: u64,
    /// 此离线租约的有效期截止 Unix 时间戳（秒）。
    pub expires_timestamp: u64,
}

/// Response returned by the marketplace `/api/v1/rest/check-license` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckLicenseResponse {
    /// Whether the license is valid and active for this plugin.
    pub valid: bool,
    /// 人类可读的状态（"valid"、"invalid"、"revoked"）。
    pub status: String,
}

/// 插件市场 `/api/v1/rest/check-update` 端点返回的响应。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckUpdateResponse {
    /// 市场上是否存在更新的稳定版本。
    pub update_available: bool,
    /// 最新的稳定版本字符串（如果存在）。
    pub latest_version: Option<String>,
}
