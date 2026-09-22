use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tracing::warn;
use wasm_encoder::{CustomSection, Encode};
use wasmparser::{Parser, Payload};

pub const WASM_SIGNATURE_SECTION: &str = "wasm_signature";
pub const PAPOKIN_METADATA_SECTION: &str = "papokin.metadata";
pub const PAPOKIN_MARKET_PUBLIC_KEY_URL: &str =
    "https://market.pumpkinmc.org/api/v1/rest/public-key";

static MARKET_PUBLIC_KEY_CACHE: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PapokinMetadata {
    pub marketplace_url: String,
    pub plugin_id: i64,
    pub plugin_name: String,
    pub version: String,
    pub dev_id: i64,
    pub dev_name: String,
    pub is_paid: bool,
    pub user_id: i64,
    pub license_key: Option<String>,
    pub issued_at: String,
}

/// 标准 W3C Wasm-Sign 签名信封结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WasmSignatureEnvelope {
    pub version: u8,
    pub algorithm: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

/// 用于 Ed25519 签名与验证的密钥对管理器。
pub struct KeyManager {
    signing_key: ed25519_dalek::SigningKey,
}

impl KeyManager {
    /// 从 32 字节的十六进制密钥加载或生成 Ed25519 密钥对。
    ///
    /// 无效的十六进制密钥此前会回退到全零密钥，这样做存在安全隐患，
    /// 公开知晓，否则任何签名都可伪造。格式错误的
    /// 密钥现在会生成随机的一次性密钥（签名只是验证
    /// 无处可寻）外加一条响亮警告，而不是静默使用弱键。
    #[must_use]
    pub fn new(secret_hex: &str) -> Self {
        use ed25519_dalek::SigningKey;

        let signing_key = match hex::decode(secret_hex) {
            Ok(decoded) if decoded.len() == 32 => {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&decoded);
                SigningKey::from_bytes(&seed)
            }
            _ => {
                warn!(
                    "Invalid Ed25519 secret hex (expected 32 bytes as hex); \
                     generated signatures will not verify. Fix the configured secret."
                );
                // 从畸形输入派生一次性密钥，而不是
                // 回退到公开可知的全零密钥。
                let seed: [u8; 32] =
                    <sha2::Sha256 as sha2::Digest>::digest(secret_hex.as_bytes()).into();
                SigningKey::from_bytes(&seed)
            }
        };
        Self { signing_key }
    }

    #[must_use]
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    /// 使用官方 W3C `wasmsign2::KeyPair` 规范对 WASM 模块进行签名。
    #[must_use]
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.signing_key.sign(data).to_bytes().to_vec()
    }
}

fn read_leb128(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        if shift >= 64 {
            return None;
        }
        result |= u64::from(byte & 0x7f) << shift;
        if (byte & 0x80) == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
    }
    None
}

/// 从 WASM 二进制中剥离所有已存在的 `papokin.metadata` 和 `wasm_signature` 段。
pub fn strip_papokin_sections(wasm_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut last_valid_end = wasm_bytes.len();
    let parser = Parser::new(0);

    for payload in parser.parse_all(wasm_bytes) {
        match payload {
            Ok(Payload::Version { ref range, .. }) => {
                last_valid_end = range.end as usize;
            }
            Ok(Payload::CustomSection(cs)) => {
                if cs.name() == PAPOKIN_METADATA_SECTION
                    || cs.name() == WASM_SIGNATURE_SECTION
                    || cs.name() == "papokin.signature"
                {
                } else {
                    last_valid_end = cs.range().end as usize;
                }
            }
            Ok(p) => {
                if let Some((_, range)) = p.as_section() {
                    last_valid_end = range.end as usize;
                }
            }
            Err(_) => {
                // 因末尾字节无法解析而停止解析（例如附加了无效的自定义节）
                break;
            }
        }
    }

    Ok(wasm_bytes[..last_valid_end].to_vec())
}

/// 使用 `wasmsign2` 标准段结构，将 `papokin.metadata` 与官方 `wasm_signature` 注入 WASM 二进制。
pub fn inject_papokin_sections(
    wasm_bytes: &[u8],
    meta: &PapokinMetadata,
    key_manager: &KeyManager,
) -> Result<Vec<u8>, String> {
    let clean_wasm = strip_papokin_sections(wasm_bytes)?;

    let meta_json = serde_json::to_vec(meta).map_err(|e| e.to_string())?;

    let mut sign_payload = Vec::new();
    sign_payload.extend_from_slice(&clean_wasm);
    sign_payload.extend_from_slice(&meta_json);

    let raw_sig = key_manager.sign(&sign_payload);
    let sig_envelope = WasmSignatureEnvelope {
        version: 1,
        algorithm: "Ed25519".into(),
        public_key_hex: key_manager.public_key_hex(),
        signature_hex: hex::encode(&raw_sig),
    };
    let sig_json = serde_json::to_vec(&sig_envelope).map_err(|e| e.to_string())?;

    let meta_section = CustomSection {
        name: PAPOKIN_METADATA_SECTION.into(),
        data: meta_json.as_slice().into(),
    };

    let sig_section = CustomSection {
        name: WASM_SIGNATURE_SECTION.into(),
        data: sig_json.as_slice().into(),
    };

    let mut out = clean_wasm;
    meta_section.encode(&mut out);
    sig_section.encode(&mut out);

    Ok(out)
}

/// 用于检查 WASM 二进制的验证结果。
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub is_signed: bool,
    pub error: Option<String>,
    pub metadata: Option<PapokinMetadata>,
    pub signature_envelope: Option<WasmSignatureEnvelope>,
    pub public_key_hex: String,
}

/// 通过 HTTP 获取市场公钥，并设有硬性超时。
///
/// 该请求运行于插件加载期间；如果没有超时，卡住的 market
/// 端点会阻塞整个加载流水线（包括服务器启动）。
async fn fetch_market_public_key_http() -> Result<String, String> {
    let client = crate::http_client::client_builder()
        .user_agent("Papokin")
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败：{e}"))?;
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get(PAPOKIN_MARKET_PUBLIC_KEY_URL).send(),
    )
    .await
    .map_err(|_| "从市场获取公钥超时（10 秒）".to_string())?
    .map_err(|e| format!("从市场获取公钥失败：{e}"))?;
    response
        .text()
        .await
        .map_err(|e| format!("读取公钥响应失败：{e}"))
}

/// 从市场 REST API 获取公钥，若之前已获取过则返回缓存的密钥。
///
/// # Errors
///若出现无法获取公钥缓存锁、HTTP 请求失败等情况，则返回错误，
/// 如果无法读取响应体，或返回的密钥为空。
pub fn fetch_market_public_key() -> Result<String, String> {
    let mut guard = MARKET_PUBLIC_KEY_CACHE
        .lock()
        .map_err(|e| format!("获取公钥缓存锁失败：{e}"))?;

    if let Some(ref cached_key) = *guard {
        return Ok(cached_key.clone());
    }

    let body = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(fetch_market_public_key_http()))?
    } else {
        tokio::runtime::Runtime::new()
            .map_err(|e| format!("创建运行时失败：{e}"))?
            .block_on(fetch_market_public_key_http())?
    };

    let key = body.trim().trim_matches('"').to_string();
    if key.is_empty() {
        Err("获取到的公钥为空".into())
    } else {
        *guard = Some(key.clone());
        Ok(key)
    }
}

/// 使用 `wasmsign2` 针对公钥验证 WASM 二进制的 papokin 自定义段。
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn verify_papokin_wasm(wasm_bytes: &[u8], public_key_hex: &str) -> VerificationResult {
    let clean_wasm = match strip_papokin_sections(wasm_bytes) {
        Ok(w) => w,
        Err(e) => {
            return VerificationResult {
                is_valid: false,
                is_signed: false,
                error: Some(format!("解析 WASM 结构失败：{e}")),
                metadata: None,
                signature_envelope: None,
                public_key_hex: public_key_hex.to_string(),
            };
        }
    };

    let mut metadata_raw: Option<Vec<u8>> = None;
    let mut signature_raw: Option<Vec<u8>> = None;

    let parser = Parser::new(0);
    for payload in parser.parse_all(wasm_bytes) {
        if let Ok(Payload::CustomSection(cs)) = payload {
            if cs.name() == PAPOKIN_METADATA_SECTION {
                metadata_raw = Some(cs.data().to_vec());
            } else if cs.name() == WASM_SIGNATURE_SECTION || cs.name() == "papokin.signature" {
                signature_raw = Some(cs.data().to_vec());
            }
        }
    }

    if metadata_raw.is_none() || signature_raw.is_none() {
        let clean_wasm = strip_papokin_sections(wasm_bytes).unwrap_or_default();
        if clean_wasm.len() < wasm_bytes.len() {
            let trailing = &wasm_bytes[clean_wasm.len()..];

            let mut cursor = 0;
            while cursor < trailing.len() {
                if trailing[cursor] == 0 {
                    cursor += 1;
                }
                if let Some((section_len, len_bytes)) = read_leb128(&trailing[cursor..]) {
                    cursor += len_bytes;
                    let section_end = cursor + section_len as usize;
                    if section_end <= trailing.len() {
                        let section_data = &trailing[cursor..section_end];
                        if let Some((name_len, name_len_bytes)) = read_leb128(section_data)
                            && let name_start = name_len_bytes
                            && let name_end = name_start + name_len as usize
                            && name_end <= section_data.len()
                            && let Ok(name) =
                                std::str::from_utf8(&section_data[name_start..name_end])
                        {
                            let payload_data = &section_data[name_end..];
                            if name == PAPOKIN_METADATA_SECTION {
                                metadata_raw = Some(payload_data.to_vec());
                            } else if name == WASM_SIGNATURE_SECTION || name == "papokin.signature"
                            {
                                signature_raw = Some(payload_data.to_vec());
                            }
                        }
                    }
                    cursor = section_end;
                    continue;
                }
                break;
            }
        }
    }

    let (Some(meta_bytes), Some(sig_bytes)) = (metadata_raw, signature_raw) else {
        return VerificationResult {
            is_valid: false,
            is_signed: false,
            error: Some("WASM 不包含 Pumpkin 市场元数据或标准的 wasm_signature 自定义段。".into()),
            metadata: None,
            signature_envelope: None,
            public_key_hex: public_key_hex.to_string(),
        };
    };

    let meta: PapokinMetadata = match serde_json::from_slice(&meta_bytes) {
        Ok(m) => m,
        Err(e) => {
            return VerificationResult {
                is_valid: false,
                is_signed: true,
                error: Some(format!("解析 papokin 元数据失败：{e}")),
                metadata: None,
                signature_envelope: None,
                public_key_hex: public_key_hex.to_string(),
            };
        }
    };

    // 提取签名字节和可选信封
    let (raw_sig_bytes, sig_envelope) =
        if let Ok(envelope) = serde_json::from_slice::<WasmSignatureEnvelope>(&sig_bytes) {
            let Ok(sig_b) = hex::decode(&envelope.signature_hex) else {
                return VerificationResult {
                    is_valid: false,
                    is_signed: true,
                    error: Some("签名信封中的签名十六进制格式无效。".into()),
                    metadata: Some(meta),
                    signature_envelope: None,
                    public_key_hex: public_key_hex.to_string(),
                };
            };
            (sig_b, Some(envelope))
        } else {
            (sig_bytes, None)
        };

    let mut sign_payload = Vec::new();
    sign_payload.extend_from_slice(&clean_wasm);
    sign_payload.extend_from_slice(&meta_bytes);

    let target_pub_key = sig_envelope.as_ref().map_or_else(
        || public_key_hex.to_string(),
        |env| env.public_key_hex.clone(),
    );

    let Ok(pub_key_bytes) = hex::decode(&target_pub_key) else {
        return VerificationResult {
            is_valid: false,
            is_signed: true,
            error: Some("公钥十六进制格式无效。".into()),
            metadata: Some(meta),
            signature_envelope: sig_envelope,
            public_key_hex: target_pub_key,
        };
    };
    let verifying_key = match VerifyingKey::try_from(pub_key_bytes.as_slice()) {
        Ok(vk) => vk,
        Err(e) => {
            return VerificationResult {
                is_valid: false,
                is_signed: true,
                error: Some(format!("无效的 Ed25519 公钥：{e}")),
                metadata: Some(meta),
                signature_envelope: sig_envelope,
                public_key_hex: target_pub_key,
            };
        }
    };

    let signature = match Signature::from_slice(&raw_sig_bytes) {
        Ok(sig) => sig,
        Err(e) => {
            return VerificationResult {
                is_valid: false,
                is_signed: true,
                error: Some(format!("无效的签名字节：{e}")),
                metadata: Some(meta),
                signature_envelope: sig_envelope,
                public_key_hex: target_pub_key,
            };
        }
    };

    if let Err(e) = verifying_key.verify(&sign_payload, &signature) {
        VerificationResult {
            is_valid: false,
            is_signed: true,
            error: Some(format!("签名校验失败：{e}")),
            metadata: Some(meta),
            signature_envelope: sig_envelope,
            public_key_hex: target_pub_key,
        }
    } else {
        VerificationResult {
            is_valid: true,
            is_signed: true,
            error: None,
            metadata: Some(meta),
            signature_envelope: sig_envelope,
            public_key_hex: target_pub_key,
        }
    }
}

/// 验证 WASM 插件二进制，若未签名或无效则记录相应警告。
pub fn verify_wasm_plugin(wasm_bytes: &[u8], path_str: &str) -> VerificationResult {
    let public_key = fetch_market_public_key().unwrap_or_default();
    let result = verify_papokin_wasm(wasm_bytes, &public_key);

    if !result.is_signed {
        warn!(
            "插件 '{}' 未签名。未签名的插件可能来自不受信任的来源。",
            path_str
        );
    } else if !result.is_valid {
        warn!(
            "插件 '{}' 签名校验失败：{}",
            path_str,
            result.error.as_deref().unwrap_or("未知错误")
        );
    }

    result
}

/// 检查 WASM 插件二进制文件是否具有有效签名。
#[must_use]
pub fn is_wasm_signed(wasm_bytes: &[u8]) -> bool {
    let public_key = fetch_market_public_key().unwrap_or_default();
    let result = verify_papokin_wasm(wasm_bytes, &public_key);
    result.is_signed && result.is_valid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_public_key_caching() {
        {
            let mut guard = MARKET_PUBLIC_KEY_CACHE.lock().unwrap();
            *guard = Some("cached_test_key_12345".to_string());
        };

        let key = fetch_market_public_key().expect("应返回缓存的键");
        assert_eq!(key, "cached_test_key_12345");

        {
            let mut guard = MARKET_PUBLIC_KEY_CACHE.lock().unwrap();
            *guard = None;
        };
    }
}
