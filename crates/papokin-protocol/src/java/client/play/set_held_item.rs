use papokin_data::packet::clientbound::play::{SET_CARRIED_ITEM, SET_HELD_SLOT};
use papokin_util::version::JavaMinecraftVersion;

use crate::ClientPacket;
use crate::packet::MultiVersionJavaPacket;
use crate::ser::NetworkWriteExt;

pub struct CSetSelectedSlot {
    pub slot: i8,
}

impl CSetSelectedSlot {
    #[must_use]
    pub const fn new(slot: i8) -> Self {
        Self { slot }
    }
}

impl ClientPacket for CSetSelectedSlot {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if *version >= JavaMinecraftVersion::V_1_21_4 {
            write.write_var_int(&crate::VarInt(i32::from(self.slot)))?;
        } else {
            write.write_i8(self.slot)?;
        }
        Ok(())
    }
}

impl MultiVersionJavaPacket for CSetSelectedSlot {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        // 26.2 将 set_carried_item 更名为 set_held_slot，
        // 生成表中的两个常量各只覆盖一个时代，须按版本选择
        if version >= JavaMinecraftVersion::V_26_2 {
            SET_HELD_SLOT.to_id(version)
        } else {
            SET_CARRIED_ITEM.to_id(version)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 数值钉死为 assets/packet/<版本>_packets.json 的权威 id；
    // 旧实现按 == V_1_21 选择，1.21.11 下落到 SET_HELD_SLOT 得到 -1，
    // 选中槽位同步包被静默丢弃
    #[test]
    fn id_matches_authoritative_table_per_era() {
        assert_eq!(
            CSetSelectedSlot::to_id(JavaMinecraftVersion::V_1_21_11),
            103
        );
        assert_eq!(CSetSelectedSlot::to_id(JavaMinecraftVersion::V_26_1), 105);
        assert_eq!(CSetSelectedSlot::to_id(JavaMinecraftVersion::V_26_2), 105);
        assert_eq!(CSetSelectedSlot::to_id(JavaMinecraftVersion::V_26_3), 107);
    }
}
