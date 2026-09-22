use std::io::Write;

use papokin_data::packet::clientbound::play::PLAYER_POSITION;
use papokin_macros::java_packet;
use papokin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, PositionFlag, ServerPacket, VarInt, WritingError, ser::NetworkReadExt,
    ser::NetworkWriteExt,
};

/// 在客户端上更新玩家的位置与旋转。
///
/// 通常被称为“传送数据包”，由服务器发送以
/// 强制改变玩家的位置。客户端必须回复一个
/// 与 `teleport_id` 匹配的 `Teleport Confirm` 数据包。
#[java_packet(PLAYER_POSITION)]
pub struct CPlayerPosition {
    /// 此传送的唯一 ID。客户端必须原样返回该 ID
    /// 以确认传送已被处理。
    pub teleport_id: VarInt,
    /// 绝对或相对的目标位置。
    pub position: Vector3<f64>,
    /// 玩家传送后的预期速度。
    pub delta: Vector3<f64>,
    /// 水平旋转角（0-360 度）。
    pub yaw: f32,
    /// 垂直旋转角（-90 到 90 度）。
    pub pitch: f32,
    /// 一组标志，用于确定上述哪些字段是相对的（~）。
    pub relatives: Vec<PositionFlag>,
}

impl CPlayerPosition {
    #[must_use]
    pub const fn new(
        teleport_id: VarInt,
        position: Vector3<f64>,
        delta: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        relatives: Vec<PositionFlag>,
    ) -> Self {
        Self {
            teleport_id,
            position,
            delta,
            yaw,
            pitch,
            relatives,
        }
    }
}

// TODO: 我们需要自定义实现吗？
impl ClientPacket for CPlayerPosition {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if version >= &JavaMinecraftVersion::V_1_21_2 {
            // 在 1.21.2 中重新排序并添加了 delta/int 标志
            write.write_var_int(&self.teleport_id)?;
            write.write_f64_be(self.position.x)?;
            write.write_f64_be(self.position.y)?;
            write.write_f64_be(self.position.z)?;
            write.write_f64_be(self.delta.x)?;
            write.write_f64_be(self.delta.y)?;
            write.write_f64_be(self.delta.z)?;
            write.write_f32_be(self.yaw)?;
            write.write_f32_be(self.pitch)?;
            write.write_i32_be(PositionFlag::get_bitfield(self.relatives.as_slice()))?;
        } else {
            write.write_f64_be(self.position.x)?;
            write.write_f64_be(self.position.y)?;
            write.write_f64_be(self.position.z)?;
            write.write_f32_be(self.yaw)?;
            write.write_f32_be(self.pitch)?;
            if version >= &JavaMinecraftVersion::V_1_8 {
                // 1.8 中加入的相对坐标标志
                write.write_u8(PositionFlag::get_bitfield(self.relatives.as_slice()) as u8)?;
            } else {
                // 1.7.x：on_ground 布尔值
                write.write_bool(false)?;
            }
            if version >= &JavaMinecraftVersion::V_1_9 {
                // 1.9 新增的传送确认 ID
                write.write_var_int(&self.teleport_id)?;
            }
            if *version >= JavaMinecraftVersion::V_1_17
                && *version <= JavaMinecraftVersion::V_1_19_3
            {
                write.write_bool(false)?;
            }
        }
        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CPlayerPosition {
    fn read(
        read: &mut &'a [u8],
        version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ser::ReadingError> {
        if version >= &JavaMinecraftVersion::V_1_21_2 {
            let teleport_id = read.get_var_int()?;
            let x = read.get_f64_be()?;
            let y = read.get_f64_be()?;
            let z = read.get_f64_be()?;
            let dx = read.get_f64_be()?;
            let dy = read.get_f64_be()?;
            let dz = read.get_f64_be()?;
            let yaw = read.get_f32_be()?;
            let pitch = read.get_f32_be()?;
            let relatives_bits = read.get_i32_be()?;
            Ok(Self {
                teleport_id,
                position: Vector3::new(x, y, z),
                delta: Vector3::new(dx, dy, dz),
                yaw,
                pitch,
                relatives: PositionFlag::from_bitfield(relatives_bits),
            })
        } else {
            let x = read.get_f64_be()?;
            let y = read.get_f64_be()?;
            let z = read.get_f64_be()?;
            let yaw = read.get_f32_be()?;
            let pitch = read.get_f32_be()?;
            let relatives = if version >= &JavaMinecraftVersion::V_1_8 {
                let relatives_bits = i32::from(read.get_u8()?);
                PositionFlag::from_bitfield(relatives_bits)
            } else {
                let _on_ground = read.get_bool()?;
                Vec::new()
            };
            let teleport_id = if version >= &JavaMinecraftVersion::V_1_9 {
                read.get_var_int()?
            } else {
                VarInt(0)
            };
            if version >= &JavaMinecraftVersion::V_1_20_2 {
                let _ = read.get_bool()?;
            }
            Ok(Self {
                teleport_id,
                position: Vector3::new(x, y, z),
                delta: Vector3::new(0.0, 0.0, 0.0),
                yaw,
                pitch,
                relatives,
            })
        }
    }
}
