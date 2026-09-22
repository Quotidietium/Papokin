use std::io::Write;

use papokin_data::packet::clientbound::login::CUSTOM_QUERY;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{
    ClientPacket, VarInt,
    ser::{NetworkWriteExt, WritingError},
};

/// 由服务器发送，用于在登录期间发起自定义插件消息交换。
///
/// 服务器软件（如代理或反作弊）用它来请求
/// 在玩家正式加入之前获取来自客户端模组的信息。
#[java_packet(CUSTOM_QUERY)]
pub struct CLoginPluginRequest<'a> {
    /// 此请求的唯一 ID。客户端必须包含这个相同的 ID
    /// 包含在响应中，以便服务器能够将它们匹配起来。
    pub message_id: VarInt,
    /// 自定义通道的名称（例如 "velocity:main"）。
    pub channel: &'a str,
    /// 原始负载数据。与标准插件消息不同，此数据
    /// 在数据包末尾序列化时通常不带长度前缀。
    pub data: &'a [u8],
}

impl<'a> CLoginPluginRequest<'a> {
    #[must_use]
    pub const fn new(message_id: VarInt, channel: &'a str, data: &'a [u8]) -> Self {
        Self {
            message_id,
            channel,
            data,
        }
    }
}

impl ClientPacket for CLoginPluginRequest<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        write.write_var_int(&self.message_id)?;

        write.write_string(self.channel)?;

        write.write_all(self.data).map_err(WritingError::IoError)
    }
}
