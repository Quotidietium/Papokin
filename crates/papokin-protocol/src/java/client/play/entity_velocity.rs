use std::io::Write;

use papokin_data::packet::clientbound::play::SET_ENTITY_MOTION;
use papokin_macros::java_packet;
use papokin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, VarInt, WritingError, codec::lp_vector_3d::LpVector3d, ser::NetworkWriteExt,
};

/// 更新实体的速度。
///
/// 此数据包告知客户端实体移动发生的突然变化，
/// 例如攻击带来的击退、爆炸，或被活塞弹射。
#[java_packet(SET_ENTITY_MOTION)]
pub struct CEntityVelocity {
    /// 速度被设置的实体 ID
    pub entity_id: VarInt,
    /// 速度向量
    pub velocity: LpVector3d,
}

impl CEntityVelocity {
    #[must_use]
    pub const fn new(entity_id: VarInt, velocity: Vector3<f64>) -> Self {
        Self {
            entity_id,
            velocity: LpVector3d(velocity),
        }
    }
}

impl ClientPacket for CEntityVelocity {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        if *version <= JavaMinecraftVersion::V_1_7_6 {
            write.write_i32_be(self.entity_id.0)?;
        } else {
            write.write_var_int(&self.entity_id)?;
        }

        // 协议 773+ 使用打包速度；772 及以下使用三个 i16 分量。
        if version >= &JavaMinecraftVersion::V_1_21_9 {
            self.velocity.write(&mut write)?;
        } else {
            self.velocity.write_legacy(&mut write)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use papokin_util::version::JavaMinecraftVersion;

    use super::CEntityVelocity;
    use crate::{
        ClientPacket, VarInt,
        codec::lp_vector_3d::{LpVector3d, encode_legacy_velocity_component},
    };

    fn encode_packet(version: JavaMinecraftVersion) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let packet = CEntityVelocity::new(
            VarInt(1),
            papokin_util::math::vector3::Vector3::new(0.5, -0.5, 0.25),
        );
        let mut out = Vec::new();
        packet.write_packet_data(&mut out, &version)?;
        Ok(out)
    }

    fn legacy_bytes(x: f64, y: f64, z: f64) -> Vec<u8> {
        let mut out = Vec::with_capacity(6);
        out.extend_from_slice(&encode_legacy_velocity_component(x).to_be_bytes());
        out.extend_from_slice(&encode_legacy_velocity_component(y).to_be_bytes());
        out.extend_from_slice(&encode_legacy_velocity_component(z).to_be_bytes());
        out
    }

    #[test]
    fn entity_velocity_uses_legacy_format_for_1_21_8() -> Result<(), Box<dyn std::error::Error>> {
        // V_1_21_7 枚举变体表示协议 772（由 1.21.7 和 1.21.8 使用）。
        let encoded = encode_packet(JavaMinecraftVersion::V_1_21_7)?;
        let expected_legacy_tail = legacy_bytes(0.5, -0.5, 0.25);

        assert_eq!(encoded, [&[1], expected_legacy_tail.as_slice()].concat());
        Ok(())
    }

    #[test]
    fn entity_velocity_uses_packed_format_for_1_21_9() -> Result<(), Box<dyn std::error::Error>> {
        let encoded = encode_packet(JavaMinecraftVersion::V_1_21_9)?;
        let legacy_like = legacy_bytes(0.5, -0.5, 0.25);

        assert_ne!(&encoded[1..], legacy_like.as_slice());

        // 确保打包后的字节仍可解码回速度
        let mut cursor = std::io::Cursor::new(&encoded[1..]);
        let decoded = LpVector3d::read(&mut cursor)?;
        assert!(decoded.0.x.is_finite());
        assert!(decoded.0.y.is_finite());
        assert!(decoded.0.z.is_finite());
        Ok(())
    }
}
