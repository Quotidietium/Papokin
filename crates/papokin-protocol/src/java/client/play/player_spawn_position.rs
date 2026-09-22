use std::io::Write;

use papokin_data::packet::clientbound::play::SET_DEFAULT_SPAWN_POSITION;
use papokin_macros::java_packet;
use papokin_util::{math::position::BlockPos, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, ServerPacket,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError},
};

/// 由服务器发送，用于设置客户端的默认出生点和指南针目标。
///
/// 此数据包更新玩家死亡后的重生位置（在未设置床或重生锚时）
/// 并决定指南针将指向的坐标。
#[java_packet(SET_DEFAULT_SPAWN_POSITION)]
pub struct CPlayerSpawnPosition {
    /// 维度的命名空间 ID（例如 "minecraft:overworld"）。
    /// 客户端需要此项来判断生成点是否位于其当前世界中。
    /// (1.21.9+)
    pub dimension_name: String,
    /// 生成位置的 X、Y、Z 坐标。
    pub location: BlockPos,
    /// 重生时玩家镜头应朝向的水平旋转角（0-360 度）。
    /// (1.17+)
    pub yaw: f32,
    /// 重生时玩家视角应朝向的垂直旋转角（-90 到 90 度）。
    /// (1.21.9+)
    pub pitch: f32,
}

impl CPlayerSpawnPosition {
    #[must_use]
    pub const fn new(location: BlockPos, yaw: f32, pitch: f32, dimension_name: String) -> Self {
        Self {
            dimension_name,
            location,
            yaw,
            pitch,
        }
    }
}

impl ClientPacket for CPlayerSpawnPosition {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if *version >= JavaMinecraftVersion::V_1_21_9 {
            write.write_string(&self.dimension_name)?;
        }

        if *version >= JavaMinecraftVersion::V_1_8 {
            write.write_block_pos(&self.location, version)?;
        } else {
            write.write_i32_be(self.location.0.x)?;
            write.write_i32_be(self.location.0.y)?;
            write.write_i32_be(self.location.0.z)?;
        }

        if *version >= JavaMinecraftVersion::V_1_17 {
            write.write_f32_be(self.yaw)?;
        }

        if *version >= JavaMinecraftVersion::V_1_21_9 {
            write.write_f32_be(self.pitch)?;
        }

        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CPlayerSpawnPosition {
    fn read(read: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let dimension_name = if *version >= JavaMinecraftVersion::V_1_21_9 {
            read.get_str()?.into_string()
        } else {
            String::new()
        };

        let location = read.get_block_pos(version)?;

        let yaw = if *version >= JavaMinecraftVersion::V_1_17 {
            read.get_f32_be()?
        } else {
            0.0
        };

        let pitch = if *version >= JavaMinecraftVersion::V_1_21_9 {
            read.get_f32_be()?
        } else {
            0.0
        };

        Ok(Self {
            dimension_name,
            location,
            yaw,
            pitch,
        })
    }
}
