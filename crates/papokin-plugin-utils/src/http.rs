//! HTTP client helpers for online license checks and marketplace queries.

use thiserror::Error;

/// HTTP 请求错误。
#[derive(Debug, Error)]
pub enum HttpError {
    /// 网络或连接错误。
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),
    /// 响应状态码不成功（非 2xx）。
    #[error("HTTP response returned error status {0}: {1}")]
    BadStatus(u16, String),
    /// 读取响应正文时出错。
    #[error("Failed to read HTTP response body: {0}")]
    BodyRead(String),
}

/// 用于查询 Pumpkin 市场 REST API 的辅助客户端。
pub struct HttpClient {
    client: reqwest::blocking::Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new("Pumpkin-Plugin-Utils/0.1.0")
    }
}

impl HttpClient {
    /// 创建带有指定 User-Agent 头的新 HTTP 客户端。
    #[must_use]
    pub fn new(user_agent: &str) -> Self {
        // reqwest 以 `rustls-no-provider` 构建；安装 ring 提供者（
        // 工作区其余部分所用的那一种）在任何客户端构建之前。
        let _ = rustls::crypto::ring::default_provider().install_default();
        let builder = reqwest::blocking::Client::builder().user_agent(user_agent);
        #[cfg(target_os = "android")]
        let builder = {
            let certs = webpki_root_certs::TLS_SERVER_ROOT_CERTS
                .iter()
                .filter_map(|c| reqwest::Certificate::from_der(c.as_ref()).ok());
            builder.tls_certs_only(certs)
        };
        let client = builder.build().unwrap_or_default();
        Self { client }
    }

    /// 执行 HTTP GET 请求并以字符串形式返回响应体。
    ///
    /// # Errors
    ///
    ///若请求失败或返回非 2xx 状态码，则返回 `HttpError`。
    pub fn get(&self, url: &str) -> Result<String, HttpError> {
        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| HttpError::RequestFailed(e.to_string()))?;

        let status = response.status().as_u16();
        if status < 200 || status >= 300 {
            let body = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpError::BadStatus(status, body));
        }

        response
            .text()
            .map_err(|e| HttpError::BodyRead(e.to_string()))
    }

    /// 执行携带 JSON 负载的 HTTP POST 请求并以字符串形式返回响应体。
    ///
    /// # Errors
    ///
    ///若请求失败或返回非 2xx 状态码，则返回 `HttpError`。
    pub fn post_json(&self, url: &str, json_payload: &str) -> Result<String, HttpError> {
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(json_payload.to_string())
            .send()
            .map_err(|e| HttpError::RequestFailed(e.to_string()))?;

        let status = response.status().as_u16();
        if status < 200 || status >= 300 {
            let body = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpError::BadStatus(status, body));
        }

        response
            .text()
            .map_err(|e| HttpError::BodyRead(e.to_string()))
    }
}
