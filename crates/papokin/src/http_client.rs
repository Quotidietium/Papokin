//! HTTP 客户端工具。

/// 创建一个配置了适当根证书的 `reqwest::ClientBuilder`。
///
/// 在 Android 上，默认的 `rustls-platform-verifier` 假定运行在 Android 应用运行时（JVM/JNI）中
/// 且在以独立二进制运行时（例如在 Termux 中）会 panic，此项会配置构建器
/// 使用来自 `webpki-root-certs` 的 Mozilla 根证书。
pub fn client_builder() -> reqwest::ClientBuilder {
    // reqwest 以 `rustls-no-provider` 构建；安装 ring 提供者（
    // 工作区其余部分所用的那一种）在任何客户端构建之前。
    let _ = rustls::crypto::ring::default_provider().install_default();
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    let builder = {
        let certs = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|c| reqwest::Certificate::from_der(c.as_ref()).ok());
        builder.tls_certs_only(certs)
    };
    builder
}

/// 创建默认的 `reqwest::Client`。
#[must_use]
pub fn client() -> reqwest::Client {
    client_builder().build().unwrap_or_default()
}
