use std::io::Write;

use papokin_data::packet::clientbound::play::CUSTOM_PAYLOAD;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{
    ClientPacket,
    ser::{NetworkWriteExt, WritingError},
};

/// 一个用于服务器与客户端之间自定义通信的数据包。
#[java_packet(CUSTOM_PAYLOAD)]
pub struct CCustomPayload<'a> {
    pub channel: &'a str,
    pub data: &'a [u8],
}

impl<'a> CCustomPayload<'a> {
    #[must_use]
    pub const fn new(channel: &'a str, data: &'a [u8]) -> Self {
        Self { channel, data }
    }
}

impl ClientPacket for CCustomPayload<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        write.write_string(self.channel)?;

        write.write_all(self.data).map_err(WritingError::IoError)
    }
}
