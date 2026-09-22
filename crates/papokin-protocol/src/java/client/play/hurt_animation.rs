use papokin_data::packet::clientbound::play::HURT_ANIMATION;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 触发实体的“受伤”视觉效果。
///
/// 此数据包使实体变红并执行一次方向性的
/// 相机震动或模型倾斜。通常紧接其后立即发送
/// 实体的生命值减少。
#[java_packet(HURT_ANIMATION)]
pub struct CHurtAnimation {
    /// 受伤的实体 ID。
    pub entity_id: VarInt,
    /// 伤害来源的偏航角（方向）。
    /// 这决定实体模型倾斜的方向。
    pub yaw: f32,
}

impl CHurtAnimation {
    #[must_use]
    pub const fn new(entity_id: VarInt, yaw: f32) -> Self {
        Self { entity_id, yaw }
    }
}

impl ClientPacket for CHurtAnimation {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.entity_id)?;
        write.write_f32_be(self.yaw)?;
        Ok(())
    }
}
