use papokin_data::packet::serverbound::config::CLIENT_INFORMATION;
use papokin_macros::java_packet;

use crate::VarInt;

use crate::{
    ServerPacket,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};
use papokin_util::version::JavaMinecraftVersion;

/// 由客户端发送，用于告知服务器其本地设置
#[java_packet(CLIENT_INFORMATION)]
pub struct SClientInformationConfig<'a> {
    /// 客户端使用的语言代码（例如 "`en_us`"）
    pub locale: &'a str,
    /// 客户端渲染的最大区块数
    pub view_distance: i8,
    /// 聊天消息的可见性（0：启用，1：仅命令，2：隐藏）
    pub chat_mode: VarInt,
    /// 客户端是否希望渲染聊天颜色/格式
    pub chat_colors: bool,
    /// 表示要显示的皮肤部位的位掩码（如披风、上衣、袖子）
    pub skin_parts: u8,
    /// 玩家的主手（0：左手，1：右手）
    pub main_hand: VarInt,
    /// 客户端是否希望启用文本过滤（例如过滤脏话）
    pub text_filtering: bool,
    /// 玩家是否应出现在服务器的在线玩家列表中
    pub server_listing: bool,
}

impl<'a> ServerPacket<'a> for SClientInformationConfig<'a> {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let locale = bytebuf.get_str_borrowed()?;
        let view_distance = bytebuf.get_i8()?;
        let chat_mode = bytebuf.get_var_int()?;
        let chat_colors = bytebuf.get_bool()?;
        let skin_parts = bytebuf.get_u8()?;
        let main_hand = if version >= &JavaMinecraftVersion::V_1_9 {
            bytebuf.get_var_int()?
        } else {
            VarInt(1)
        };
        let text_filtering = if version >= &JavaMinecraftVersion::V_1_17 {
            bytebuf.get_bool()?
        } else {
            false
        };
        let server_listing = if version >= &JavaMinecraftVersion::V_1_18 {
            bytebuf.get_bool()?
        } else {
            true
        };

        Ok(Self {
            locale,
            view_distance,
            chat_mode,
            chat_colors,
            skin_parts,
            main_hand,
            text_filtering,
            server_listing,
        })
    }
}

impl crate::ClientPacket for SClientInformationConfig<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_string(self.locale)?;
        write.write_i8(self.view_distance)?;
        write.write_var_int(&self.chat_mode)?;
        write.write_bool(self.chat_colors)?;
        write.write_u8(self.skin_parts)?;
        if version >= &JavaMinecraftVersion::V_1_9 {
            write.write_var_int(&self.main_hand)?;
        }
        if version >= &JavaMinecraftVersion::V_1_17 {
            write.write_bool(self.text_filtering)?;
        }
        if version >= &JavaMinecraftVersion::V_1_18 {
            write.write_bool(self.server_listing)?;
        }
        Ok(())
    }
}
