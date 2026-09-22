use std::io::Write;

use crate::{
    ClientPacket,
    ser::{NetworkWriteExt, WritingError},
};
use papokin_data::block_state_remap::remap_block_state_for_version;
use papokin_data::packet::clientbound::play::LEVEL_EVENT;
use papokin_data::world::WorldEvent;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;

use papokin_macros::java_packet;

/// 由服务器发送，用于在世界中的某处触发特定的声音或粒子效果。
///
/// 这用于各种各样的效果，从破坏方块到烟花
/// 从爆炸到水花飞溅或唱片播放。
#[java_packet(LEVEL_EVENT)]
pub struct CWorldEvent {
    /// 要触发的事件 ID（例如 1000 表示射箭，2001 表示方块破坏）。
    /// 音效/粒子 ID 的完整列表请参阅最新的协议注册表。
    pub event: i32,
    /// 效果应当起始的世界坐标。
    pub location: BlockPos,
    /// 与该事件关联的附加元数据。
    ///
    /// 例如，破坏方块时，其中包含方块 ID。
    /// 对于烟花粒子，它可能包含颜色或类型。
    pub data: i32,
    /// 如果为 true，声音将以恒定音量播放，无论
    /// 玩家与 `location` 的距离。
    pub disable_relative_volume: bool,
}

impl CWorldEvent {
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

impl ClientPacket for CWorldEvent {
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
