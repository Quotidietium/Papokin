use papokin_data::packet::serverbound::config::COOKIE_RESPONSE;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{
    ReadingError, ServerPacket,
    ser::{NetworkReadExt, NetworkReadSliceExt},
};

/// cookie 负载允许的最大大小（5 KiB）。
const MAX_COOKIE_LENGTH: usize = 5120;

/// 配置阶段对来自服务器的 `CCookieRequest` 的响应
///
/// Cookie 允许服务器在客户端一侧存储少量数据，
/// 之后可以取回（例如用于会话跟踪或偏好设置）
#[java_packet(COOKIE_RESPONSE)]
pub struct SConfigCookieResponse<'a> {
    /// 返回的 cookie 的唯一标识符
    pub key: &'a str,
    /// 表示此响应是否附带负载
    pub has_payload: bool,
    /// cookie 中实际存储的数据。上限为 5120 字节
    pub payload: Option<&'a [u8]>,
}

impl<'a> ServerPacket<'a> for SConfigCookieResponse<'a> {
    fn read(read: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let key = read.get_str_borrowed()?;
        let has_payload = read.get_bool()?;

        if !has_payload {
            return Ok(Self {
                key,
                has_payload,
                payload: None,
            });
        }

        let payload_length = read.get_var_int()?.0 as usize;
        if payload_length > MAX_COOKIE_LENGTH {
            return Err(ReadingError::TooLarge("SConfigCookieResponse".to_string()));
        }

        let payload = read.read_slice_borrowed(payload_length)?;
        Ok(Self {
            key,
            has_payload,
            payload: Some(payload),
        })
    }
}

impl crate::ClientPacket for SConfigCookieResponse<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::{VarInt, ser::NetworkWriteExt};
        write.write_string(self.key)?;
        if let Some(payload) = self.payload {
            write.write_bool(true)?;
            write.write_var_int(&VarInt(payload.len() as i32))?;
            write.write_slice(payload)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}
