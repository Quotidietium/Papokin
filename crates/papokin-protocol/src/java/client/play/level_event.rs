use std::io::Write;

use crate::{
    ClientPacket,
    ser::{NetworkWriteExt, WritingError},
};
use papokin_data::block_state_remap::remap_block_state_for_version;
use papokin_data::packet::clientbound::play::LEVEL_EVENT;
use papokin_data::world::WorldEvent;
use papokin_macros::java_packet;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;

/// 在世界位置触发特定的音效或粒子效果。
///
/// 此数据包处理各种各样的“世界级”事件，例如
/// 方块破坏粒子、烟花爆炸或环境音效
/// 例如门打开、传送门嗡嗡作响等。
#[java_packet(LEVEL_EVENT)]
pub struct CLevelEvent {
    /// 要触发的事件 ID。
    /// 事件 ID 通常分为声音事件（1000 号段）和
    /// 粒子/视觉事件（2000 系列）。
    pub event: i32,
    /// 事件发生的世界坐标。
    pub location: BlockPos,
    /// 事件特定的数据（例如破坏粒子所用的方块 ID
    /// 或烟雾团的方向）。
    pub data: i32,
    /// 如果为 true，声音以恒定音量播放，无论
    /// 玩家与 `location` 的距离。
    pub disable_relative_volume: bool,
}

impl CLevelEvent {
    #[must_use]
    pub const fn new(
        event: i32,
        location: BlockPos,
        data: i32,
        disable_relative_volume: bool,
    ) -> Self {
        Self {
            event,
            location,
            data,
            disable_relative_volume,
        }
    }
}

impl ClientPacket for CLevelEvent {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;
        write.write_i32_be(self.event)?;
        write.write_block_pos(&self.location, version)?;

        let data = if self.event == WorldEvent::ParticlesDestroyBlock as i32 {
            u16::try_from(self.data).map_or(self.data, |state_id| {
                i32::from(remap_block_state_for_version(state_id, *version))
            })
        } else {
            self.data
        };
        write.write_i32_be(data)?;
        write.write_bool(self.disable_relative_volume)?;

        Ok(())
    }
}
