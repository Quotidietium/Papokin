use papokin_data::packet::clientbound::play::SET_SCORE;
use papokin_util::text::TextComponent;

use papokin_macros::java_packet;

use crate::{
    ClientPacket, NumberFormat, VarInt,
    ser::{NetworkWriteExt, WritingError},
};

/// 由服务器发送，用于在特定记分板目标上为实体创建或更新分数。
///
/// 此数据包是管理记分板数据的主要方式。在最新协议中，
/// 它还支持以自定义格式显示数值分数的可选项。
#[java_packet(SET_SCORE)]
pub struct CUpdateScore {
    /// 正在更新分数的实体名称（例如玩家的用户名
    /// 或“Kills”之类的非玩家条目）。
    pub entity_name: String,
    /// 此分数所属目标的内部名称。
    pub objective_name: String,
    /// 分数的实际整数值。
    pub value: VarInt,
    /// 可选的实体自定义名称，将显示在记分板中。
    /// 若为 `None`，则默认使用 `entity_name`。
    pub display_name: Option<TextComponent>,
    /// 数值的可选格式（例如空白、固定文本或带样式）。
    /// 这允许分数以原始数字以外的形式显示。
    pub number_format: Option<NumberFormat>,
}

impl CUpdateScore {
    #[must_use]
    pub const fn new(
        entity_name: String,
        objective_name: String,
        value: VarInt,
        display_name: Option<TextComponent>,
        number_format: Option<NumberFormat>,
    ) -> Self {
        Self {
            entity_name,
            objective_name,
            value,
            display_name,
            number_format,
        }
    }

    #[must_use]
    pub const fn new_remove(entity_name: String, objective_name: String) -> Self {
        Self {
            entity_name,
            objective_name,
            value: VarInt(0),
            display_name: None,
            number_format: None,
        }
    }
}

impl ClientPacket for CUpdateScore {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &papokin_util::version::JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        write.write_string(&self.entity_name)?;
        if *version >= papokin_util::version::JavaMinecraftVersion::V_1_20_3 {
            write.write_string(&self.objective_name)?;
            write.write_var_int(&self.value)?;
            write.write_option(&self.display_name, |w, t| w.write_component(t, version))?;
            write.write_option(&self.number_format, |w, n| n.write(w))
        } else if *version <= papokin_util::version::JavaMinecraftVersion::V_1_7_6 {
            write.write_u8(0)?;
            write.write_string(&self.objective_name)?;
            write.write_i32_be(self.value.0)
        } else {
            write.write_u8(0)?;
            write.write_string(&self.objective_name)?;
            write.write_var_int(&self.value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_util::version::JavaMinecraftVersion;

    #[test]
    fn update_score_serialization() {
        let packet = CUpdateScore::new("Alex".into(), "kills".into(), VarInt(42), None, None);

        // 新版 1.20.3+
        let mut buf_modern = Vec::new();
        packet
            .write_packet_data(&mut buf_modern, &JavaMinecraftVersion::V_1_20_3)
            .unwrap();

        // 1.8 - 1.20.2
        let mut buf_legacy = Vec::new();
        packet
            .write_packet_data(&mut buf_legacy, &JavaMinecraftVersion::V_1_8)
            .unwrap();

        // 1.7.6
        let mut buf_v1_7 = Vec::new();
        packet
            .write_packet_data(&mut buf_v1_7, &JavaMinecraftVersion::V_1_7_6)
            .unwrap();

        assert!(!buf_modern.is_empty());
        assert!(!buf_legacy.is_empty());
        assert!(!buf_v1_7.is_empty());
    }
}
