//! 以非阻塞方式对市场 `/api/v1/rest/check-update` 端点进行更新检查。

use crate::{
    http::{HttpClient, HttpError},
    models::CheckUpdateResponse,
};
use thiserror::Error;
use tracing::debug;

/// 更新检查错误。
#[derive(Debug, Error)]
pub enum UpdateError {
    /// 插件尚未初始化。
    #[error(
        "Plugin-utils has not been initialized (call papokin_plugin_utils::init(context) first)"
    )]
    NotInitialized,
    /// 查询更新端点时的 HTTP 错误。
    #[error("Failed to query update API: {0}")]
    Http(#[from] HttpError),
    /// 来自响应的 JSON 解析错误。
    #[error("Failed to parse update response JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// 通过 Pumpkin Marketplace API 检查插件更新。
pub struct UpdateChecker {
    http_client: HttpClient,
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateChecker {
    /// 创建新的 `UpdateChecker`。
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::default(),
        }
    }

    /// 检查市场上是否存在更新的版本：
    /// `GET /api/v1/rest/check-update?plugin_name={name}&current_version={version}`
    ///
    /// # Errors
    ///
    ///若网络请求或 JSON 解析失败，则返回 `UpdateError`。
    pub fn check_for_updates(
        &self,
        plugin_name: &str,
        current_version_str: &str,
        marketplace_url: &str,
    ) -> Result<CheckUpdateResponse, UpdateError> {
        let url = format!(
            "{}/api/v1/rest/check-update?plugin_name={}&current_version={}",
            marketplace_url.trim_end_matches('/'),
            urlencoding(plugin_name),
            urlencoding(current_version_str),
        );
        debug!("正在检查更新：{url}");

        let response_str = self.http_client.get(&url)?;
        let update_response: CheckUpdateResponse = serde_json::from_str(&response_str)?;

        Ok(update_response)
    }
}

/// 用于查询参数的极简 URL 编码辅助函数。
fn urlencoding(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}
