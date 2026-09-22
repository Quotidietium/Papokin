use papokin_data::packet::serverbound::config::RESOURCE_PACK;
use papokin_macros::java_packet;

use crate::{
    ServerPacket,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};
use papokin_util::version::JavaMinecraftVersion;

use crate::VarInt;

pub enum ResourcePackResponseResult {
    DownloadSuccess,
    DownloadFail,
    Downloaded,
    Accepted,
    Declined,
    InvalidUrl,
    ReloadFailed,
    Discarded,
    Unknown(i32),
}

/// 由客户端发送，用于告知服务器所请求资源包的状态。
///
/// 这让服务器能够知道玩家是否在使用所需的纹理
/// 或下载失败时。
#[java_packet(RESOURCE_PACK)]
pub struct SConfigResourcePack {
    /// 此响应所指资源包的唯一标识符。
    pub uuid: uuid::Uuid,
    /// 操作的状态码，映射到 [`ResourcePackResponseResult`]。
    pub result: VarInt,
}

impl<'a> ServerPacket<'a> for SConfigResourcePack {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let uuid = if *version >= JavaMinecraftVersion::V_1_20_3 {
            bytebuf.get_uuid()?
        } else {
            uuid::Uuid::nil()
        };
        if *version < JavaMinecraftVersion::V_1_10 {
            let _hash = bytebuf.get_str_bounded_borrowed(40)?;
        }
        let result = bytebuf.get_var_int()?;
        Ok(Self { uuid, result })
    }
}

impl crate::ClientPacket for SConfigResourcePack {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_uuid(&self.uuid)?;
        write.write_var_int(&self.result)?;
        Ok(())
    }
}

impl SConfigResourcePack {
    #[must_use]
    pub const fn response_result(&self) -> ResourcePackResponseResult {
        match self.result.0 {
            0 => ResourcePackResponseResult::DownloadSuccess,
            1 => ResourcePackResponseResult::Declined,
            2 => ResourcePackResponseResult::DownloadFail,
            3 => ResourcePackResponseResult::Accepted,
            4 => ResourcePackResponseResult::Downloaded,
            5 => ResourcePackResponseResult::InvalidUrl,
            6 => ResourcePackResponseResult::ReloadFailed,
            7 => ResourcePackResponseResult::Discarded,
            x => ResourcePackResponseResult::Unknown(x),
        }
    }
}
