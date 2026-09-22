use papokin_data::packet::clientbound::play::PLAYER_ABILITIES;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 更新玩家的移动与交互能力。
///
/// 此数据包告知客户端玩家的状态（飞行、无敌）
/// 并设置移动速度。客户端虽然会应用这些视觉效果，
/// 服务器仍必须校验这些状态以防止作弊。
#[java_packet(PLAYER_ABILITIES)]
pub struct CPlayerAbilities {
    /// 玩家状态的位掩码。
    /// 位 0 (0x01)：无敌（创造模式）
    /// 位 1 (0x02)：飞行中
    /// 位 2 (0x04)：允许飞行
    /// 位 3 (0x08)：创造模式（瞬间破坏）
    pub flags: i8,
    /// 飞行速度的乘数。
    /// 默认值为 0.05。
    pub flying_speed: f32,
    /// 视野修正值（行走速度倍率）。
    /// 默认值为 0.1。
    pub field_of_view: f32,
}

impl CPlayerAbilities {
    #[must_use]
    pub const fn new(flags: i8, flying_speed: f32, field_of_view: f32) -> Self {
        Self {
            flags,
            flying_speed,
            field_of_view,
        }
    }
}

impl ClientPacket for CPlayerAbilities {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_i8(self.flags)?;
        write.write_f32_be(self.flying_speed)?;
        write.write_f32_be(self.field_of_view)?;
        Ok(())
    }
}
