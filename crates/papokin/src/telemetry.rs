use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use papokin_config::TelemetryConfig;
use serde::{Deserialize, Serialize};

use crate::server::Server;

/// 用于在重启间持久化遥测身份密钥的文件名。
pub const IDENTITY_KEY_PATH: &str = ".papokin/identity.key";

/// 用于在重启间持久化遥测身份密钥的备用文件名。
pub const FALLBACK_IDENTITY_KEY_PATH: &str = "telemetry_key.bin";

/// 以小写十六进制格式传输 Ed25519 公钥的 HTTP 头名称。
pub const HEADER_PUBLIC_KEY: &str = "X-Telemetry-Public-Key";

/// 以小写十六进制格式传输 Ed25519 签名的 HTTP 头名称。
pub const HEADER_SIGNATURE: &str = "X-Telemetry-Signature";

/// 以 Unix 秒数传输请求时间戳的 HTTP 头名称。
pub const HEADER_TIMESTAMP: &str = "X-Telemetry-Timestamp";

/// 遥测客户端与服务器之间允许的最大时钟漂移（300 秒 / 5 分钟）。
pub const MAX_CLOCK_DRIFT_SECS: u64 = 300;

/// 服务器上已启用插件的信息。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PluginInfo {
    /// 插件名称。
    pub name: String,
    /// 插件的版本。
    pub version: String,
}

/// 插件遥测信息的类型别名。
pub type PluginTelemetryInfo = PluginInfo;

/// 传输到后端的遥测心跳负载。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HeartbeatPayload {
    /// 服务器软件实现类型（例如 "papokin"）。
    pub server_type: String,
    /// Papokin 服务器软件版本。
    pub server_version: String,
    /// Minecraft 游戏版本字符串。
    pub minecraft_version: String,
    /// 网络协议版本号。
    pub protocol_version: i32,
    /// 当前已连接的玩家数量。
    pub online_players: usize,
    /// 允许的最大玩家数量。
    pub max_players: u32,
    /// 宿主机操作系统名称。
    pub os: String,
    /// 宿主机 CPU 架构。
    pub arch: String,
    /// 可用 CPU 核心数。
    pub cpu_cores: usize,
    /// CPU 型号名称或品牌。
    pub cpu_model: String,
    /// 系统分配/使用的内存量（MiB）。
    pub ram_allocated_mb: u64,
    /// 系统可用的物理内存总量（MiB）。
    pub total_ram_mb: u64,
    /// 活跃插件列表。
    pub plugins: Vec<PluginInfo>,
    /// 服务器是否已选择加入公开目录列表。
    pub is_public: bool,
    /// 选择加入公共目录列表时的服务器公开名称。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_name: Option<String>,
}

/// 从遥测后端接收到的响应负载。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HeartbeatResponse {
    /// 响应状态（"ok"、"throttled" 等）。
    pub status: String,
    /// 来自后端的状态或错误消息。
    #[serde(default)]
    pub message: String,
    /// 由后端确认的服务器公钥。
    #[serde(default)]
    pub server_public_key: String,
    /// 建议在下一次心跳之前等待的秒数。
    #[serde(default)]
    pub next_ping_seconds: u32,
}

/// 计算签名数据的字节布局：`[timestamp_ascii_bytes] + [b'.'] + [raw_json_body_bytes]`。
#[must_use]
pub fn compute_signed_data(timestamp_str: &str, body_bytes: &[u8]) -> Vec<u8> {
    let mut signed_data = Vec::with_capacity(timestamp_str.len() + 1 + body_bytes.len());
    signed_data.extend_from_slice(timestamp_str.as_bytes());
    signed_data.push(b'.');
    signed_data.extend_from_slice(body_bytes);
    signed_data
}

/// 使用 Ed25519 签名密钥对遥测消息数据进行签名，并返回 `(public_key_hex, signature_hex)`。
#[must_use]
pub fn sign_telemetry_payload(
    signing_key: &SigningKey,
    timestamp_str: &str,
    body_bytes: &[u8],
) -> (String, String) {
    let signed_data = compute_signed_data(timestamp_str, body_bytes);
    let signature = signing_key.sign(&signed_data);
    let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
    let sig_hex = hex::encode(signature.to_bytes());
    (pubkey_hex, sig_hex)
}

/// 遥测请求签名验证期间可能发生的错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TelemetryVerificationError {
    #[error("时间戳字符串格式无效")]
    InvalidTimestamp,
    #[error("时间戳漂移超限：漂移 {drift} 秒（最大允许 {MAX_CLOCK_DRIFT_SECS} 秒）")]
    ClockDriftExceeded { drift: u64 },
    #[error("公钥十六进制编码无效")]
    InvalidPublicKeyHex,
    #[error("公钥字节无效：{0}")]
    InvalidPublicKey(String),
    #[error("签名十六进制编码无效")]
    InvalidSignatureHex,
    #[error("签名字节无效：{0}")]
    InvalidSignature(String),
    #[error("签名验证失败：{0}")]
    VerificationFailed(String),
}

/// 针对 Ed25519 公钥与时间戳验证已签名的遥测请求。
///
/// 确保 `current_time_secs` 与 `timestamp_str` 之间的时钟偏差不超过 `±300` 秒。
pub fn verify_telemetry_request(
    pubkey_hex: &str,
    sig_hex: &str,
    timestamp_str: &str,
    body_bytes: &[u8],
    current_time_secs: u64,
) -> Result<(), TelemetryVerificationError> {
    let ts: u64 = timestamp_str
        .parse()
        .map_err(|_| TelemetryVerificationError::InvalidTimestamp)?;

    let drift = current_time_secs.abs_diff(ts);
    if drift > MAX_CLOCK_DRIFT_SECS {
        return Err(TelemetryVerificationError::ClockDriftExceeded { drift });
    }

    let pubkey_bytes =
        hex::decode(pubkey_hex).map_err(|_| TelemetryVerificationError::InvalidPublicKeyHex)?;
    let vk = VerifyingKey::try_from(pubkey_bytes.as_slice())
        .map_err(|e| TelemetryVerificationError::InvalidPublicKey(e.to_string()))?;

    let sig_bytes =
        hex::decode(sig_hex).map_err(|_| TelemetryVerificationError::InvalidSignatureHex)?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|e| TelemetryVerificationError::InvalidSignature(e.to_string()))?;

    let signed_data = compute_signed_data(timestamp_str, body_bytes);
    vk.verify(&signed_data, &sig)
        .map_err(|e| TelemetryVerificationError::VerificationFailed(e.to_string()))?;

    Ok(())
}

/// 用于与 Pumpkin Marketplace 遥测后端（上游服务）通信的客户端。
pub struct TelemetryClient {
    pub signing_key: SigningKey,
    pub http_client: reqwest::Client,
    pub endpoint: String,
    pub api_base_url: String,
}

impl TelemetryClient {
    /// 创建新的 `TelemetryClient` 实例。
    #[must_use]
    pub fn new(signing_key: SigningKey, http_client: reqwest::Client, endpoint: String) -> Self {
        let api_base_url = endpoint
            .strip_suffix("/api/v1/rest/telemetry/heartbeat")
            .unwrap_or_else(|| {
                endpoint
                    .strip_suffix("/heartbeat")
                    .unwrap_or_else(|| endpoint.trim_end_matches('/'))
            })
            .to_string();

        Self {
            signing_key,
            http_client,
            endpoint,
            api_base_url,
        }
    }

    ///返回客户端 Ed25519 公钥的 64 字符小写十六进制表示。
    #[must_use]
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    /// 从给定路径加载已有的 Ed25519 私钥/种子，或生成并持久化一个新密钥。
    pub fn load_or_generate_key(key_path: &Path) -> Result<SigningKey, std::io::Error> {
        if key_path.exists() {
            let bytes = std::fs::read(key_path)?;
            if bytes.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&bytes);
                return Ok(SigningKey::from_bytes(&seed));
            }
            tracing::warn!(
                "无法从 {} 读取有效的 32 字节身份密钥，正在生成新密钥",
                key_path.display()
            );
        }

        // 生成新密钥
        let seed: [u8; 32] = rand::random();
        let key = SigningKey::from_bytes(&seed);

        if let Some(parent) = key_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(key_path, seed)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(key)
    }

    /// 向遥测后端发送经过签名的心跳请求。
    pub async fn send_heartbeat(
        &self,
        payload: &HeartbeatPayload,
    ) -> Result<HeartbeatResponse, reqwest::Error> {
        #[allow(clippy::expect_used)]
        let body_bytes = serde_json::to_vec(payload).expect("序列化失败");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp_str = timestamp.to_string();

        let (pubkey_hex, sig_hex) =
            sign_telemetry_payload(&self.signing_key, &timestamp_str, &body_bytes);

        let response = self
            .http_client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header(HEADER_PUBLIC_KEY, pubkey_hex)
            .header(HEADER_SIGNATURE, sig_hex)
            .header(HEADER_TIMESTAMP, timestamp_str)
            .body(body_bytes)
            .send()
            .await?
            .error_for_status()?
            .json::<HeartbeatResponse>()
            .await?;

        Ok(response)
    }

    /// 向遥测后端发送经过签名的服务器关停通知。
    pub async fn send_shutdown(&self, reason: Option<&str>) -> Result<(), reqwest::Error> {
        let payload = serde_json::json!({
            "reason": reason.unwrap_or("server shutdown")
        });
        #[allow(clippy::expect_used)]
        let body_bytes = serde_json::to_vec(&payload).expect("序列化失败");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp_str = timestamp.to_string();

        let (pubkey_hex, sig_hex) =
            sign_telemetry_payload(&self.signing_key, &timestamp_str, &body_bytes);

        let _ = self
            .http_client
            .post(format!(
                "{}/api/v1/rest/telemetry/shutdown",
                self.api_base_url
            ))
            .header("Content-Type", "application/json")
            .header(HEADER_PUBLIC_KEY, pubkey_hex)
            .header(HEADER_SIGNATURE, sig_hex)
            .header(HEADER_TIMESTAMP, timestamp_str)
            .body(body_bytes)
            .send()
            .await;

        Ok(())
    }
}

/// 通过检查默认位置或生成新密钥来解析服务器身份密钥。
#[must_use]
pub fn resolve_identity_key() -> SigningKey {
    let primary = Path::new(IDENTITY_KEY_PATH);
    if primary.exists()
        && let Ok(key) = TelemetryClient::load_or_generate_key(primary)
    {
        return key;
    }

    let fallback = Path::new(FALLBACK_IDENTITY_KEY_PATH);
    if fallback.exists()
        && let Ok(key) = TelemetryClient::load_or_generate_key(fallback)
    {
        return key;
    }

    let data_primary = Path::new("data").join(IDENTITY_KEY_PATH);
    if data_primary.exists()
        && let Ok(key) = TelemetryClient::load_or_generate_key(&data_primary)
    {
        return key;
    }

    let data_fallback = Path::new("data").join(FALLBACK_IDENTITY_KEY_PATH);
    if data_fallback.exists()
        && let Ok(key) = TelemetryClient::load_or_generate_key(&data_fallback)
    {
        return key;
    }

    let data_dir_key = Path::new("data").join(IDENTITY_KEY_PATH);
    if Path::new("data").is_dir()
        && let Ok(key) = TelemetryClient::load_or_generate_key(&data_dir_key)
    {
        return key;
    }

    TelemetryClient::load_or_generate_key(primary).unwrap_or_else(|err| {
        tracing::warn!(
            "无法在 {} 持久保存遥测身份密钥：{err}，将使用临时密钥",
            primary.display()
        );
        let seed: [u8; 32] = rand::random();
        SigningKey::from_bytes(&seed)
    })
}

/// 辅助函数：在指定路径加载或创建身份密钥。
#[must_use]
pub fn get_or_create_identity_key(path: &Path) -> SigningKey {
    TelemetryClient::load_or_generate_key(path).unwrap_or_else(|err| {
        tracing::warn!(
            "无法将身份密钥保存到 {}：{err}，将使用临时密钥",
            path.display()
        );
        let seed: [u8; 32] = rand::random();
        SigningKey::from_bytes(&seed)
    })
}

/// 根据当前服务器与运行时状态构建遥测心跳负载。
#[must_use]
pub fn build_heartbeat_payload(
    server: &Server,
    telemetry_config: &TelemetryConfig,
) -> HeartbeatPayload {
    let os = sysinfo::System::long_os_version().unwrap_or_else(|| {
        let name = sysinfo::System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
        sysinfo::System::os_version().map_or(name.clone(), |ver| format!("{name} {ver}"))
    });
    let arch = std::env::consts::ARCH.to_string();
    let cpu_cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);

    let (ram_allocated_mb, total_ram_mb, cpu_model) = if sysinfo::IS_SUPPORTED_SYSTEM {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        let ram_allocated = sys.used_memory() / (1024 * 1024);
        let total_ram = sys.total_memory() / (1024 * 1024);
        let cpu_model = sys.cpus().first().map_or_else(
            || "Unknown".to_string(),
            |c| {
                let brand = c.brand().trim();
                if brand.is_empty() {
                    "Unknown".to_string()
                } else {
                    brand.to_string()
                }
            },
        );
        (ram_allocated, total_ram, cpu_model)
    } else {
        (0, 0, "Unknown".to_string())
    };

    let plugins = server
        .plugin_manager
        .active_plugins()
        .into_iter()
        .map(|p| PluginInfo {
            name: p.name,
            version: p.version,
        })
        .collect();

    let is_public = telemetry_config.public;
    let public_name = if is_public {
        telemetry_config.server_name.clone()
    } else {
        None
    };

    HeartbeatPayload {
        server_type: "papokin".to_string(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        minecraft_version: papokin_data::packet::CURRENT_MC_VERSION.to_string(),
        protocol_version: papokin_data::packet::CURRENT_MC_VERSION.protocol_version(),
        online_players: server.get_player_count(),
        max_players: server.advanced_config.networking.java.max_players,
        os,
        arch,
        cpu_cores,
        cpu_model,
        ram_allocated_mb,
        total_ram_mb,
        plugins,
        is_public,
        public_name,
    }
}

/// 使用服务器的遥测配置启动后台遥测上报任务。
pub fn start_telemetry(server: Arc<Server>) {
    let config = server.telemetry_config.clone();
    start_telemetry_with_config(server, &config);
}

/// 以指定配置启动后台遥测上报任务。
#[allow(clippy::needless_pass_by_value)]
pub fn start_telemetry_with_config(server: Arc<Server>, config: &TelemetryConfig) {
    if !config.enabled {
        tracing::trace!("遥测已在配置中禁用。");
        return;
    }
    if config.endpoint.trim().is_empty() {
        tracing::warn!("遥测已启用但 endpoint 为空，拒绝启动遥测上报。");
        return;
    }

    tracing::info!(
        "匿名服务器遥测已启用。正在向 {} 定期发送心跳",
        config.endpoint
    );

    let user_agent = format!("Papokin-Server/{}", env!("CARGO_PKG_VERSION"));
    let http_client = match crate::http_client::client_builder()
        .timeout(Duration::from_secs(10))
        .user_agent(&user_agent)
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            tracing::debug!("创建遥测 HTTP 客户端失败：{err}");
            return;
        }
    };

    let signing_key = resolve_identity_key();
    let telemetry_client = Arc::new(TelemetryClient::new(
        signing_key,
        http_client,
        config.endpoint.clone(),
    ));

    let interval_secs = config.interval_secs.max(60);
    let config = config.clone();
    let server_task = server.clone();

    server.spawn_task(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        while !crate::SHOULD_STOP.load(Ordering::Relaxed) {
            tokio::select! {
                _ = interval.tick() => {}
                () = crate::STOP_INTERRUPT.cancelled() => {
                    break;
                }
            }
            if crate::SHOULD_STOP.load(Ordering::Relaxed) {
                break;
            }

            let payload = build_heartbeat_payload(&server_task, &config);

            match telemetry_client.send_heartbeat(&payload).await {
                Ok(resp) => {
                    tracing::trace!(
                        "遥测心跳发送成功（status: {}, public_key: {}）",
                        resp.status,
                        resp.server_public_key
                    );
                }
                Err(err) => {
                    tracing::debug!("遥测心跳发送失败：{err}");
                }
            }
        }

        // 在服务器终止 / SIGTERM 时触发关停遥测
        if let Err(err) = telemetry_client
            .send_shutdown(Some("server shutdown"))
            .await
        {
            tracing::debug!("遥测关停消息发送失败：{err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::post;

    #[test]
    fn identity_key_generation_and_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(".papokin/identity.key");

        assert!(!key_path.exists());
        let key1 = TelemetryClient::load_or_generate_key(&key_path).unwrap();
        assert!(key_path.exists());

        // 检查密钥文件为 32 字节
        let file_bytes = std::fs::read(&key_path).unwrap();
        assert_eq!(file_bytes.len(), 32);

        #[cfg(unix)]
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(
                &std::fs::metadata(&key_path).unwrap().permissions()
            ) & 0o777,
            0o600
        );

        // 重载现有键
        let key2 = TelemetryClient::load_or_generate_key(&key_path).unwrap();
        assert_eq!(
            key1.verifying_key().to_bytes(),
            key2.verifying_key().to_bytes()
        );

        // 损坏文件测试（少于 32 字节）
        std::fs::write(&key_path, b"corrupted-key-data").unwrap();
        let key3 = TelemetryClient::load_or_generate_key(&key_path).unwrap();
        assert_ne!(
            key1.verifying_key().to_bytes(),
            key3.verifying_key().to_bytes()
        );
        let new_file_bytes = std::fs::read(&key_path).unwrap();
        assert_eq!(new_file_bytes.len(), 32);
    }

    #[test]
    fn heartbeat_payload_serialization() {
        let payload = HeartbeatPayload {
            server_type: "papokin".to_string(),
            server_version: "0.3.2".to_string(),
            minecraft_version: "1.21.4".to_string(),
            protocol_version: 769,
            online_players: 42,
            max_players: 100,
            os: "Ubuntu 24.04 LTS".to_string(),
            arch: "x86_64".to_string(),
            cpu_cores: 8,
            cpu_model: "AMD Ryzen 7 5800X 8-Core Processor".to_string(),
            ram_allocated_mb: 8192,
            total_ram_mb: 16384,
            plugins: vec![
                PluginInfo {
                    name: "Essential Admin Tools".to_string(),
                    version: "1.0.0".to_string(),
                },
                PluginInfo {
                    name: "PapokinAuth".to_string(),
                    version: "1.2.0".to_string(),
                },
            ],
            is_public: true,
            public_name: Some("My High-Performance Papokin SMP".to_string()),
        };

        let json_str = serde_json::to_string_pretty(&payload).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // 确保 server_id 被完全移除
        assert!(parsed.get("server_id").is_none());

        assert_eq!(parsed["server_type"], "papokin");
        assert_eq!(parsed["server_version"], "0.3.2");
        assert_eq!(parsed["minecraft_version"], "1.21.4");
        assert_eq!(parsed["protocol_version"], 769);
        assert_eq!(parsed["online_players"], 42);
        assert_eq!(parsed["max_players"], 100);
        assert_eq!(parsed["os"], "Ubuntu 24.04 LTS");
        assert_eq!(parsed["arch"], "x86_64");
        assert_eq!(parsed["cpu_cores"], 8);
        assert_eq!(parsed["cpu_model"], "AMD Ryzen 7 5800X 8-Core Processor");
        assert_eq!(parsed["ram_allocated_mb"], 8192);
        assert_eq!(parsed["total_ram_mb"], 16384);
        assert_eq!(parsed["is_public"], true);
        assert_eq!(parsed["public_name"], "My High-Performance Papokin SMP");
        assert_eq!(parsed["plugins"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn heartbeat_response_deserialization() {
        let json_data = r#"{
            "status": "ok",
            "message": "Heartbeat accepted",
            "server_public_key": "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "next_ping_seconds": 300
        }"#;

        let resp: HeartbeatResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.message, "Heartbeat accepted");
        assert_eq!(
            resp.server_public_key,
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
        );
        assert_eq!(resp.next_ping_seconds, 300);
    }

    #[test]
    fn message_signing_and_verification() {
        let seed = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let timestamp_str = "1757185000";
        let body = br#"{"server_type":"papokin","online_players":5}"#;

        // 验证签名数据格式："{timestamp}.{body}"
        let signed_data = compute_signed_data(timestamp_str, body);
        let expected_prefix = format!("{timestamp_str}.");
        assert!(signed_data.starts_with(expected_prefix.as_bytes()));
        assert_eq!(&signed_data[expected_prefix.len()..], body);

        // 告示牌负载
        let (pubkey_hex, sig_hex) = sign_telemetry_payload(&signing_key, timestamp_str, body);
        assert_eq!(pubkey_hex.len(), 64);
        assert_eq!(sig_hex.len(), 128);

        // 所有十六进制字符必须为小写
        assert_eq!(pubkey_hex, pubkey_hex.to_lowercase());
        assert_eq!(sig_hex, sig_hex.to_lowercase());

        // 用匹配的时间戳进行验证
        let current_time: u64 = 1757185100; // 100 秒漂移（在 300 秒内）
        let verify_result =
            verify_telemetry_request(&pubkey_hex, &sig_hex, timestamp_str, body, current_time);
        assert!(verify_result.is_ok());

        // 漂移超限（> 300 秒）
        let expired_time: u64 = 1757185000 + 301;
        let expired_result =
            verify_telemetry_request(&pubkey_hex, &sig_hex, timestamp_str, body, expired_time);
        assert!(matches!(
            expired_result,
            Err(TelemetryVerificationError::ClockDriftExceeded { .. })
        ));

        // 损坏的签名
        let mut corrupted_sig = sig_hex.clone();
        let flipped_char = if corrupted_sig.starts_with('0') {
            '1'
        } else {
            '0'
        };
        corrupted_sig.replace_range(0..1, &flipped_char.to_string());
        let corrupted_result = verify_telemetry_request(
            &pubkey_hex,
            &corrupted_sig,
            timestamp_str,
            body,
            current_time,
        );
        assert!(matches!(
            corrupted_result,
            Err(TelemetryVerificationError::VerificationFailed(_))
        ));

        // 损坏的主体
        let tampered_body = br#"{"server_type":"papokin","online_players":6}"#;
        let tampered_result = verify_telemetry_request(
            &pubkey_hex,
            &sig_hex,
            timestamp_str,
            tampered_body,
            current_time,
        );
        assert!(matches!(
            tampered_result,
            Err(TelemetryVerificationError::VerificationFailed(_))
        ));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn telemetry_client_send_heartbeat_and_shutdown() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let seed = [7u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let expected_pubkey = hex::encode(signing_key.verifying_key().to_bytes());

        // 搭建 mock axum 服务器
        let app = Router::new()
            .route(
                "/api/v1/rest/telemetry/heartbeat",
                post(move |req: Request| async move {
                    let headers = req.headers().clone();
                    let pubkey = headers.get(HEADER_PUBLIC_KEY).and_then(|v| v.to_str().ok());
                    let signature = headers.get(HEADER_SIGNATURE).and_then(|v| v.to_str().ok());
                    let timestamp = headers.get(HEADER_TIMESTAMP).and_then(|v| v.to_str().ok());

                    let (Some(pubkey), Some(sig), Some(ts)) = (pubkey, signature, timestamp) else {
                        return (StatusCode::UNAUTHORIZED, "Missing required headers")
                            .into_response();
                    };

                    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                        .await
                        .unwrap();

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    if verify_telemetry_request(pubkey, sig, ts, &body, now).is_err() {
                        return (StatusCode::UNAUTHORIZED, "Invalid signature").into_response();
                    }

                    let response = HeartbeatResponse {
                        status: "ok".to_string(),
                        message: "Heartbeat accepted".to_string(),
                        server_public_key: pubkey.to_string(),
                        next_ping_seconds: 300,
                    };
                    let json_bytes = serde_json::to_vec(&response).unwrap();
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        json_bytes,
                    )
                        .into_response()
                }),
            )
            .route(
                "/api/v1/rest/telemetry/shutdown",
                post(move |req: Request| async move {
                    let headers = req.headers().clone();
                    let pubkey = headers.get(HEADER_PUBLIC_KEY).and_then(|v| v.to_str().ok());
                    let signature = headers.get(HEADER_SIGNATURE).and_then(|v| v.to_str().ok());
                    let timestamp = headers.get(HEADER_TIMESTAMP).and_then(|v| v.to_str().ok());

                    let (Some(pubkey), Some(sig), Some(ts)) = (pubkey, signature, timestamp) else {
                        return (StatusCode::UNAUTHORIZED, "Missing headers").into_response();
                    };

                    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                        .await
                        .unwrap();

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    if verify_telemetry_request(pubkey, sig, ts, &body, now).is_err() {
                        return (StatusCode::UNAUTHORIZED, "Invalid signature").into_response();
                    }

                    (StatusCode::OK, "Shutdown received").into_response()
                }),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let endpoint = format!("http://127.0.0.1:{port}/api/v1/rest/telemetry/heartbeat");
        let http_client = crate::http_client::client_builder().build().unwrap();
        let client = TelemetryClient::new(signing_key, http_client, endpoint);

        let payload = HeartbeatPayload {
            server_type: "papokin".to_string(),
            server_version: "0.1.0".to_string(),
            minecraft_version: "1.21.4".to_string(),
            protocol_version: 769,
            online_players: 1,
            max_players: 20,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            cpu_cores: 4,
            cpu_model: "Test CPU".to_string(),
            ram_allocated_mb: 1024,
            total_ram_mb: 2048,
            plugins: vec![],
            is_public: false,
            public_name: None,
        };

        // 发送有效心跳 -> 200 OK
        let resp = client.send_heartbeat(&payload).await.unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.message, "Heartbeat accepted");
        assert_eq!(resp.server_public_key, expected_pubkey);
        assert_eq!(resp.next_ping_seconds, 300);

        // 发送有效关停 -> 200 OK
        let shutdown_res = client.send_shutdown(Some("maintenance")).await;
        assert!(shutdown_res.is_ok());

        // 测试无效签名 / 未授权
        let body_bytes = serde_json::to_vec(&payload).unwrap();
        let timestamp_str = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let response = crate::http_client::client_builder()
            .build()
            .unwrap()
            .post(format!(
                "http://127.0.0.1:{port}/api/v1/rest/telemetry/heartbeat"
            ))
            .header("Content-Type", "application/json")
            .header(HEADER_PUBLIC_KEY, &expected_pubkey)
            .header(HEADER_SIGNATURE, "00".repeat(64)) // 无效签名
            .header(HEADER_TIMESTAMP, timestamp_str)
            .body(body_bytes)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    }
}
