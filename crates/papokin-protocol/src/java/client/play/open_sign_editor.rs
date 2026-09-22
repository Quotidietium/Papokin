use papokin_data::packet::clientbound::play::OPEN_SIGN_EDITOR;
use papokin_macros::java_packet;
use papokin_util::math::position::BlockPos;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 为客户端打开告示牌文本输入界面。
///
/// 服务器发送此数据包以强制客户端显示
/// 告示牌编辑界面。这通常紧接着
/// 玩家放置告示牌或与现有告示牌交互时（如果允许）。
#[java_packet(OPEN_SIGN_EDITOR)]
pub struct COpenSignEditor {
    /// 待编辑的告示牌方块的世界坐标。
    pub location: BlockPos,
    /// 编辑器应打开告示牌的正面还是背面。
    /// 在 1.20“足迹与故事”更新中引入，用于双面告示牌。
    pub is_front_text: bool,
}

impl COpenSignEditor {
    #[must_use]
    pub const fn new(location: BlockPos, is_front_text: bool) -> Self {
        Self {
            location,
            is_front_text,
        }
    }
}

impl ClientPacket for COpenSignEditor {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if *version <= JavaMinecraftVersion::V_1_7_6 {
            write.write_i32_be(self.location.0.x)?;
            write.write_i32_be(self.location.0.y)?;
            write.write_i32_be(self.location.0.z)?;
        } else {
            write.write_block_pos(&self.location, version)?;
        }

        if *version >= JavaMinecraftVersion::V_1_20 {
            write.write_bool(self.is_front_text)?;
        }
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for COpenSignEditor {
    fn read(
        bytebuf: &mut &'a [u8],
        version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ser::ReadingError> {
        use crate::ser::NetworkReadExt;
        let location = if *version <= JavaMinecraftVersion::V_1_7_6 {
            let x = bytebuf.get_i32_be()?;
            let y = bytebuf.get_i32_be()?;
            let z = bytebuf.get_i32_be()?;
            BlockPos::new(x, y, z)
        } else {
            bytebuf.get_block_pos(version)?
        };

        let is_front_text = if *version >= JavaMinecraftVersion::V_1_20 {
            bytebuf.get_bool()?
        } else {
            true
        };

        Ok(Self {
            location,
            is_front_text,
        })
    }
}
