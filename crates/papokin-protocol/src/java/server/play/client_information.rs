use crate::{
    ServerPacket,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};
use papokin_data::packet::serverbound::play::CLIENT_INFORMATION;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::VarInt;

#[java_packet(CLIENT_INFORMATION)]
pub struct SClientInformationPlay<'a> {
    pub locale: &'a str, // 16
    pub view_distance: i8,
    pub chat_mode: VarInt, // VarInt
    pub chat_colors: bool,
    pub skin_parts: u8,
    pub main_hand: VarInt,
    pub text_filtering: bool,
    pub server_listing: bool,
    /// 客户端的粒子显示偏好（0：全部，1：减少，2：最少；1.21.2 加入）
    pub particle_status: VarInt,
}

impl<'a> ServerPacket<'a> for SClientInformationPlay<'a> {
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
        let particle_status = if version >= &JavaMinecraftVersion::V_1_21_2 {
            bytebuf.get_var_int()?
        } else {
            VarInt(0)
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
            particle_status,
        })
    }
}

impl crate::ClientPacket for SClientInformationPlay<'_> {
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
        if version >= &JavaMinecraftVersion::V_1_21_2 {
            write.write_var_int(&self.particle_status)?;
        }
        Ok(())
    }
}
