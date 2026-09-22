use crate::ClientPacket;
use crate::VarInt;
use crate::packet::MultiVersionJavaPacket;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::{ENTITY_POSITION_SYNC, TELEPORT_ENTITY};
use papokin_util::math::vector3::Vector3;
use papokin_util::version::JavaMinecraftVersion;

/// 更新实体的精确位置、旋转和速度。
///
/// 此数据包用于在服务器端对实体移动进行权威控制。
/// 在最新的协议版本中，它取代了多个旧的“相对移动”
/// 数据包，以提供更精确的同步并减少“橡皮筋”现象。
///
/// 注意：此数据包绝不能用于接收该数据包的玩家或
/// 玩家当前骑乘的任何实体。
pub struct CEntityPositionSync {
    /// 被移动实体的实体 ID。
    pub entity_id: VarInt,
    /// 实体在世界中的绝对位置。
    pub position: Vector3<f64>,
    /// 实体当前的速度（增量），客户端用于
    /// 用于平滑插值。
    pub delta: Vector3<f64>,
    /// 绝对偏航角（水平旋转），单位为度。
    pub yaw: f32,
    /// 绝对俯仰角（垂直旋转），单位为度。
    pub pitch: f32,
    /// 实体当前是否接触地面。
    pub on_ground: bool,
}

impl CEntityPositionSync {
    #[must_use]
    pub const fn new(
        entity_id: VarInt,
        position: Vector3<f64>,
        delta: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) -> Self {
        Self {
            entity_id,
            position,
            delta,
            yaw,
            pitch,
            on_ground,
        }
    }
}

impl MultiVersionJavaPacket for CEntityPositionSync {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_1_21_2 {
            ENTITY_POSITION_SYNC.to_id(version)
        } else {
            TELEPORT_ENTITY.to_id(version)
        }
    }
}

impl ClientPacket for CEntityPositionSync {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.entity_id)?;
        // 自 26.3 起，位置是一个路径。0 是线性路径，即仅仅是终点位置。
        if version >= &JavaMinecraftVersion::V_26_3 {
            write.write_var_int(&VarInt(0))?;
        }
        write.write_f64_be(self.position.x)?;
        write.write_f64_be(self.position.y)?;
        write.write_f64_be(self.position.z)?;
        if version >= &JavaMinecraftVersion::V_1_21_2 {
            // delta 在 26.3 中已被 path 取代。
            if version < &JavaMinecraftVersion::V_26_3 {
                write.write_f64_be(self.delta.x)?;
                write.write_f64_be(self.delta.y)?;
                write.write_f64_be(self.delta.z)?;
            }
            write.write_f32_be(self.yaw)?;
            write.write_f32_be(self.pitch)?;
        } else {
            write.write_u8((self.yaw.rem_euclid(360.0) * 256.0 / 360.0).floor() as u8)?;
            write.write_u8((self.pitch.rem_euclid(360.0) * 256.0 / 360.0).floor() as u8)?;
        }
        write.write_bool(self.on_ground)?;
        Ok(())
    }
}
