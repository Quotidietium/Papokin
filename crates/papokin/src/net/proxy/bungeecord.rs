use arc_swap::ArcSwap;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::{net::IpAddr, net::SocketAddr};
use thiserror::Error;
use tracing::warn;

use crate::net::{GameProfile, offline_uuid};
use papokin_protocol::Property;

/// `BungeeGuard` 插件用于转发其共享内容的属性名称
/// 密钥就藏在档案属性中。
const BUNGEEGUARD_TOKEN_PROPERTY: &str = "bungeeguard-token";

#[derive(Error, Debug)]
pub enum BungeeCordError {
    #[error("Failed to parse address")]
    FailedParseAddress,
    #[error("Failed to parse UUID")]
    FailedParseUUID,
    #[error("Failed to parse properties")]
    FailedParseProperties,
    #[error("Failed to make offline UUID")]
    FailedMakeOfflineUUID,
    #[error("No BungeeGuard token in forwarded data")]
    MissingToken,
    #[error("Invalid BungeeGuard token")]
    InvalidToken,
}

/// 尝试通过 `BungeeCord` 登录玩家。
///
/// 应在收到 `SLoginStart` 数据包时调用此函数。
/// 它利用在 `SHandShake` 数据包中收到的 `server_address`，
/// 其中可能包含关于客户端的可选数据：
///
/// 1. IP 地址（如果 `BungeeCord` 服务器上启用了 `ip_forward`）
/// 2. UUID（如果 `BungeeCord` 服务器上启用了 `ip_forward`）
/// 3. 游戏档案属性（如果 `BungeeCord` 服务器上启用了 `ip_forward` 和 `online_mode`）
///
/// 若配置了 `secret`，则属性中必须包含一个名为
/// `bungeeguard-token`（内含密钥），由 `BungeeGuard` 注入
/// 插件。令牌属性会从 profile 中剥离，且缺失或
/// 令牌不匹配则拒绝连接。这也会阻止
/// 直接连接到本服务器而绕过代理。
///
/// 若缺少任何可选数据，该函数将尝试
/// 在本地判定玩家的信息。
pub fn bungeecord_login(
    client_address: &SocketAddr,
    server_address: &str,
    name: String,
    secret: &str,
) -> Result<(IpAddr, GameProfile), BungeeCordError> {
    let mut parts = server_address.split('\0');

    // 跳过第一部分（实际的服务器地址/主机）
    let _host = parts.next();

    let ip = match parts.next() {
        Some(ip_str) if !ip_str.is_empty() => ip_str
            .parse()
            .map_err(|_| BungeeCordError::FailedParseAddress)?,
        _ => client_address.ip(),
    };

    let id = match parts.next() {
        Some(uuid_str) if !uuid_str.is_empty() => uuid_str
            .parse()
            .map_err(|_| BungeeCordError::FailedParseUUID)?,
        _ => offline_uuid(&name).map_err(|_| BungeeCordError::FailedMakeOfflineUUID)?,
    };

    let mut properties: Vec<Property> = match parts.next() {
        Some(json_str) if !json_str.is_empty() => {
            serde_json::from_str(json_str).map_err(|_| BungeeCordError::FailedParseProperties)?
        }
        _ => Vec::new(),
    };

    // `BungeeGuard` 插件将共享密钥注入为一个名为
    // `bungeeguard-token` 位于转发的档案属性中。当
    // 密钥已配置，则该属性必须存在且持有
    // 密钥；随后该属性会被剥离，绝不会传到游戏
    // 玩家资料。这也会阻止玩家直接连接而非
    // 通过代理。
    if !secret.is_empty() {
        let token_props: Vec<&Property> = properties
            .iter()
            .filter(|property| property.name.as_ref() == BUNGEEGUARD_TOKEN_PROPERTY)
            .collect();

        match token_props.as_slice() {
            [token] if token.value.as_ref() == secret => {
                properties.retain(|property| property.name.as_ref() != BUNGEEGUARD_TOKEN_PROPERTY);
            }
            [] => {
                warn!(
                    "Rejecting login: forwarded data has no `{}` property \
                     ({} parts, property names: {:?})",
                    BUNGEEGUARD_TOKEN_PROPERTY,
                    server_address.split('\0').count(),
                    properties
                        .iter()
                        .map(|p| p.name.as_ref())
                        .collect::<Vec<_>>()
                );
                return Err(BungeeCordError::MissingToken);
            }
            _ => {
                // 只记录 SHA-256 哈希：单向，因此密钥永不泄露，
                // 但足以区分不匹配与重复令牌。
                let token_hashes: Vec<String> = token_props
                    .iter()
                    .map(|property| hex::encode(Sha256::digest(property.value.as_bytes())))
                    .collect();
                warn!(
                    "Rejecting login: expected exactly one matching `{}` property, \
                     found {} (token hashes: {token_hashes:?}, configured secret \
                     hash: {})",
                    BUNGEEGUARD_TOKEN_PROPERTY,
                    token_props.len(),
                    hex::encode(Sha256::digest(secret.as_bytes()))
                );
                return Err(BungeeCordError::InvalidToken);
            }
        }
    }

    Ok((
        ip,
        GameProfile {
            id,
            name,
            properties: ArcSwap::new(Arc::new(properties)),
            profile_actions: None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_protocol::ser::NetworkWriteExt;
    use papokin_protocol::{
        ServerPacket, codec::var_int::VarInt, java::server::handshake::SHandShake,
    };
    use papokin_util::version::JavaMinecraftVersion;

    /// 驱动代理登录的完整流程：握手被编码为
    /// `BungeeCord` 会将其送上网络线路，由真实的数据包读取器解码，
    /// 其产生的地址会交给 `bungeecord_login`。失败正在于此
    /// 当读取器为 `server_address` 设置的边界太小而无法容纳
    /// 转发的档案属性。
    #[tokio::test]
    async fn logs_in_from_a_handshake_decoded_off_the_wire() {
        let textures = "e".repeat(432);
        let signature = "s".repeat(684);
        let address = format!(
            "mc.example.com\0192.0.2.10\0d8f4a1e0-0f1b-4c3a-9f2e-1a2b3c4d5e6f\0\
             [{{\"name\":\"textures\",\"value\":\"{textures}\",\"signature\":\"{signature}\"}}]"
        );

        let mut buf = Vec::new();
        let protocol_version = JavaMinecraftVersion::V_1_21_11.protocol_version();
        buf.write_var_int(&VarInt(protocol_version))
            .expect("写入协议版本");
        buf.write_string(&address).expect("写入服务器地址");
        buf.write_u16_be(25565).expect("写入服务器端口");
        buf.write_var_int(&VarInt(2)).expect("写入下一状态");

        let handshake = SHandShake::read(&mut &buf[..], &JavaMinecraftVersion::V_1_21_11)
            .expect("BungeeCord 发送的握手应可读");

        let client_address = SocketAddr::from(([10, 0, 0, 1], 51234));
        let (ip, profile) = bungeecord_login(
            &client_address,
            &handshake.server_address,
            "Steve".to_string(),
            "",
        )
        .expect("转发地址应能生成游戏档案");

        // 使用的是转发过来的 IP 与 UUID，而非代理自己的套接字地址。
        assert_eq!(ip, IpAddr::from([192, 0, 2, 10]));
        assert_eq!(
            profile.id,
            "d8f4a1e0-0f1b-4c3a-9f2e-1a2b3c4d5e6f"
                .parse::<uuid::Uuid>()
                .expect("有效的 UUID")
        );

        // 带签名的皮肤得以保留，因此玩家维持原外观。
        let properties = profile.properties.load();
        assert_eq!(properties.len(), 1);
        assert_eq!(&*properties[0].name, "textures");
        assert_eq!(&*properties[0].value, textures.as_str());
        assert_eq!(properties[0].signature.as_deref(), Some(signature.as_str()));
    }

    const SECRET: &str = "bungeeguard-token";
    const FORWARDED_HOST: &str = concat!(
        // 在数字处分割，使 `\0` 不被读取为八进制转义。
        "mc.example.com\0",
        "192.0.2.10\0",
        "d8f4a1e0-0f1b-4c3a-9f2e-1a2b3c4d5e6f"
    );

    fn client_address() -> SocketAddr {
        SocketAddr::from(([10, 0, 0, 1], 51234))
    }

    /// 转发地址，以给定档案的 `properties` 作为其
    /// 第四部分，与 `BungeeCord` 在网络上传输的格式一致。
    fn forwarded_address(properties: &str) -> String {
        format!("{FORWARDED_HOST}\0{properties}")
    }

    /// 仅有 `BungeeGuard` 令牌属性，因为该插件会将其注入到
    /// 转发的档案属性。
    fn token_property(token: &str) -> String {
        format!(r#"{{"name":"bungeeguard-token","value":"{token}","signature":""}}"#)
    }

    /// 一个包含给定属性对象的属性数组。
    fn properties_array(properties: &[&str]) -> String {
        format!("[{}]", properties.join(","))
    }

    #[test]
    fn accepts_matching_bungeeguard_token() {
        let properties = format!(
            r#"[{{"name":"textures","value":"skin","signature":"sig"}},{{"name":"bungeeguard-token","value":"{SECRET}","signature":""}}]"#
        );
        let address = forwarded_address(&properties);

        let (ip, profile) =
            bungeecord_login(&client_address(), &address, "Steve".to_string(), SECRET)
                .expect("匹配的令牌应被接受");

        assert_eq!(ip, IpAddr::from([192, 0, 2, 10]));

        // token 属性会被剥离，因此它绝不会进入游戏档案。
        let properties = profile.properties.load();
        assert_eq!(properties.len(), 1);
        assert_eq!(&*properties[0].name, "textures");
    }

    #[test]
    fn rejects_missing_bungeeguard_token() {
        let address =
            forwarded_address(r#"[{"name":"textures","value":"skin","signature":"sig"}]"#);

        let result = bungeecord_login(&client_address(), &address, "Steve".to_string(), SECRET);

        assert!(matches!(result, Err(BungeeCordError::MissingToken)));
    }

    #[test]
    fn rejects_mismatched_bungeeguard_token() {
        let address = forwarded_address(&properties_array(&[&token_property("wrong-token")]));

        let result = bungeecord_login(&client_address(), &address, "Steve".to_string(), SECRET);

        assert!(matches!(result, Err(BungeeCordError::InvalidToken)));
    }

    #[test]
    fn rejects_multiple_bungeeguard_tokens() {
        let properties = properties_array(&[&token_property(SECRET), &token_property(SECRET)]);
        let address = forwarded_address(&properties);

        let result = bungeecord_login(&client_address(), &address, "Steve".to_string(), SECRET);

        assert!(matches!(result, Err(BungeeCordError::InvalidToken)));
    }

    #[test]
    fn rejects_direct_connection_when_secret_is_configured() {
        let result = bungeecord_login(
            &client_address(),
            "mc.example.com",
            "Steve".to_string(),
            SECRET,
        );

        assert!(matches!(result, Err(BungeeCordError::MissingToken)));
    }

    #[test]
    fn ignores_token_when_no_secret_is_configured() {
        let address = forwarded_address(&properties_array(&[&token_property(SECRET)]));

        let result = bungeecord_login(&client_address(), &address, "Steve".to_string(), "");

        assert!(
            result.is_ok(),
            "an unconfigured secret must not reject logins"
        );
    }
}
