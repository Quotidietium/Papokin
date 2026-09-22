use papokin_data::packet::clientbound::login::{GAME_PROFILE, LOGIN_FINISHED};
use papokin_util::version::JavaMinecraftVersion;

use crate::{ClientPacket, Property, packet::MultiVersionJavaPacket, ser::NetworkWriteExt};

/// 由服务器发送，用于表示登录成功并转入配置阶段
///
/// 此数据包向客户端提供其正式的 UUID 和用户名，
/// 服务器识别，连同任何关联的皮肤或披风属性。
pub struct CLoginSuccess<'a> {
    /// 分配给玩家的唯一标识符。
    pub uuid: &'a uuid::Uuid,
    /// 玩家已验证的用户名。
    pub username: &'a str,
    /// 玩家档案的属性列表，例如皮肤数据和签名。
    /// 这通常从 Mojang 身份验证服务器获取。
    pub properties: &'a [Property],
    /// (<1.21.2) 是否启用严格的错误处理。
    pub strict_error_handling: bool,
    /// 于 26.2 加入
    pub session_id: uuid::Uuid,
}

impl<'a> CLoginSuccess<'a> {
    #[must_use]
    pub const fn new(
        uuid: &'a uuid::Uuid,
        username: &'a str,
        properties: &'a [Property],
        strict_error_handling: bool,
        session_id: uuid::Uuid,
    ) -> Self {
        Self {
            uuid,
            username,
            properties,
            strict_error_handling,
            session_id,
        }
    }
}

impl MultiVersionJavaPacket for CLoginSuccess<'_> {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_1_21_2 {
            LOGIN_FINISHED.to_id(version)
        } else {
            GAME_PROFILE.to_id(version)
        }
    }
}

impl ClientPacket for CLoginSuccess<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if version < &JavaMinecraftVersion::V_1_16 {
            write.write_string(&self.uuid.to_string())?;
        } else {
            write.write_uuid(self.uuid)?;
        }
        write.write_string(self.username)?;
        if version >= &JavaMinecraftVersion::V_1_19 {
            write.write_list(self.properties, |write, property| property.write(write))?;
        }
        if version >= &JavaMinecraftVersion::V_26_2 {
            write.write_uuid(&self.session_id)?;
        }
        if version >= &JavaMinecraftVersion::V_1_20_5 && version < &JavaMinecraftVersion::V_1_21_2 {
            write.write_bool(self.strict_error_handling)?;
        }
        Ok(())
    }
}
