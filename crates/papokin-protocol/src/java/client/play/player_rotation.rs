use papokin_data::packet::clientbound::play::{MOVE_PLAYER_ROT, PLAYER_ROTATION};

use crate::{ClientPacket, packet::MultiVersionJavaPacket, ser::NetworkWriteExt};
use papokin_util::version::JavaMinecraftVersion;

pub struct CPlayerRotation {
    pub yaw: f32,
    pub pitch: f32,
}

/// 26.2 将 `move_player_rot` 更名为 `player_rotation`，生成表中的两个
/// 常量各只覆盖一个时代，须按版本选择，否则 1.x 版本下 id 为 -1。
impl MultiVersionJavaPacket for CPlayerRotation {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_26_2 {
            PLAYER_ROTATION.to_id(version)
        } else {
            MOVE_PLAYER_ROT.to_id(version)
        }
    }
}

impl CPlayerRotation {
    #[must_use]
    pub const fn new(yaw: f32, pitch: f32) -> Self {
        Self { yaw, pitch }
    }
}

impl ClientPacket for CPlayerRotation {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_f32_be(self.yaw)?;
        if *version >= JavaMinecraftVersion::V_1_21_9 {
            write.write_bool(false)?;
        }
        write.write_f32_be(self.pitch)?;
        if *version >= JavaMinecraftVersion::V_1_21_9 {
            write.write_bool(false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 数值钉死为 assets/packet/<版本>_packets.json 的权威 id：
    // 26.2 更名后若误用单一 PLAYER_ROTATION 常量，1.x 下 id 为 -1，
    // 头部旋转包被静默丢弃
    #[test]
    fn id_matches_authoritative_table_per_era() {
        assert_eq!(CPlayerRotation::to_id(JavaMinecraftVersion::V_1_21_11), 71);
        assert_eq!(CPlayerRotation::to_id(JavaMinecraftVersion::V_26_1), 73);
        assert_eq!(CPlayerRotation::to_id(JavaMinecraftVersion::V_26_2), 73);
        assert_eq!(CPlayerRotation::to_id(JavaMinecraftVersion::V_26_3), 74);
    }
}
