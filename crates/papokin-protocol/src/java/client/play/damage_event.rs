use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::DAMAGE_EVENT;
use papokin_macros::java_packet;
use papokin_util::math::vector3::Vector3;
use papokin_util::version::JavaMinecraftVersion;

/// 通知客户端有实体受到了伤害。
///
/// 此数据包用于触发受击动画（如生物身上的泛红效果），
/// 方向性击退视觉效果和音效。它向客户端提供
/// 并附带伤害来源的具体细节，以确保视觉反馈
/// 与原因匹配。
#[java_packet(DAMAGE_EVENT)]
pub struct CDamageEvent {
    /// 受到伤害的实体 ID。
    pub entity_id: VarInt,
    /// 伤害类型的 ID（引用 `minecraft:damage_type` 注册表）。
    /// 示例：`magic`、`fall`、`on_fire` 或 `arrow`。
    pub source_type_id: VarInt,
    /// 伤害实际来源的实体 ID（例如射箭的玩家）。
    /// 若没有特定的实体成因，则设为 0。
    pub source_cause_id: VarInt,
    /// 直接造成伤害者的实体 ID（例如箭实体本身）。
    /// 若与成因相同或不适用，则设为 0。
    pub source_direct_id: VarInt,
    /// 伤害来源的坐标。客户端用于计算
    /// “伤害倾斜”镜头效果的方向。
    pub source_position: Option<Vector3<f64>>,
}

impl CDamageEvent {
    #[must_use]
    pub fn new(
        entity_id: VarInt,
        source_type_id: VarInt,
        source_cause_id: Option<VarInt>,
        source_direct_id: Option<VarInt>,
        source_position: Option<Vector3<f64>>,
    ) -> Self {
        Self {
            entity_id,
            source_type_id,
            source_cause_id: source_cause_id.map_or(VarInt(0), |id| VarInt(id.0 + 1)),
            source_direct_id: source_direct_id.map_or(VarInt(0), |id| VarInt(id.0 + 1)),
            source_position,
        }
    }
}

impl ClientPacket for CDamageEvent {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        // 伤害类型是同步注册表；将数据集 id 转换为
        // 该客户端在配置阶段收到的 id 空间。原版 id
        // 经过静态重映射表；插件注册的自定义 id
        // 条目（>= 原生原版数量）按差值偏移
        // 采用原版的条目数量，或对相应客户端降级为 `generic`
        // 从未收到自定义条目。
        let source_type_id = papokin_data::damage_ext::translate_damage_type_id_for_version(
            u16::try_from(self.source_type_id.0).unwrap_or(0),
            *version,
        );
        write.write_var_int(&self.entity_id)?;
        write.write_var_int(&VarInt(i32::from(source_type_id)))?;
        write.write_var_int(&self.source_cause_id)?;
        write.write_var_int(&self.source_direct_id)?;
        if let Some(pos) = &self.source_position {
            write.write_bool(true)?;
            write.write_f64(pos.x)?;
            write.write_f64(pos.y)?;
            write.write_f64(pos.z)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::damage::DamageType;
    use papokin_data::sync_id_remap::remap_damage_type_id_for_version;

    fn serialize(version: JavaMinecraftVersion) -> Vec<u8> {
        let packet = CDamageEvent::new(
            VarInt(1),
            VarInt(i32::from(DamageType::SULFUR_CUBE_HOT.id)),
            None,
            None,
            None,
        );
        let mut buf = Vec::new();
        packet.write_packet_data(&mut buf, &version).unwrap();
        buf
    }

    #[test]
    fn damage_event_remaps_source_type_for_1_21_11() {
        // `sulfur_cube_hot` 只存在于 26.3 数据集中；1.21.11 客户端
        // 绝不能看到其原始 id。实体 id（varint 1）在最前，
        // 伤害类型 id 其次 —— 此处均为单字节 varint。
        let native = serialize(JavaMinecraftVersion::V_26_3);
        let old = serialize(JavaMinecraftVersion::V_1_21_11);

        assert_eq!(native[1], DamageType::SULFUR_CUBE_HOT.id);
        let expected = remap_damage_type_id_for_version(
            u16::from(DamageType::SULFUR_CUBE_HOT.id),
            JavaMinecraftVersion::V_1_21_11,
        );
        assert_ne!(expected, u16::from(DamageType::SULFUR_CUBE_HOT.id));
        assert_eq!(old[1], u8::try_from(expected).unwrap());
    }

    #[test]
    fn damage_event_translates_custom_ids_per_version() {
        use papokin_data::damage_ext::{
            damage_type_vanilla_count_for_version, native_damage_type_count,
        };

        // 第一个由插件注册的自定义伤害类型紧跟在
        // 原版条目（位于原生 id 空间中）。
        let native_count = native_damage_type_count();
        let packet =
            CDamageEvent::new(VarInt(1), VarInt(i32::from(native_count)), None, None, None);
        let serialize_with = |version: JavaMinecraftVersion| {
            let mut buf = Vec::new();
            packet.write_packet_data(&mut buf, &version).unwrap();
            buf
        };

        // 1.21.11 客户端在*其*原版（条目）之后接收自定义条目
        // 条目之后，而非原生条目之后。
        let vanilla_1_21_11 =
            damage_type_vanilla_count_for_version(JavaMinecraftVersion::V_1_21_11).unwrap();
        let old = serialize_with(JavaMinecraftVersion::V_1_21_11);
        // 实体 id（varint 1）在前，伤害类型 id 其次 —— 两者
        // 在这些取值下是单字节 varint。
        assert_eq!(old[1], u8::try_from(vanilla_1_21_11).unwrap());

        // 在原生 id 空间中，id 原样发送。
        let native = serialize_with(JavaMinecraftVersion::V_26_3);
        assert_eq!(native[1], u8::try_from(native_count).unwrap());

        // 注册表未通过配置阶段同步的客户端
        // 状态（1.20/1.20.1）从未收到过自定义条目，将得到
        // 版本的 `generic` id，而不是超出范围的 id。
        let legacy = serialize_with(JavaMinecraftVersion::V_1_20);
        let expected_generic = remap_damage_type_id_for_version(
            u16::from(DamageType::GENERIC.id),
            JavaMinecraftVersion::V_1_20,
        );
        assert_eq!(legacy[1], u8::try_from(expected_generic).unwrap());
    }
}
