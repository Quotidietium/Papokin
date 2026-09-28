use papokin_data::packet::serverbound::play::SET_GAME_RULE;
use papokin_macros::java_packet;

use crate::{
    ServerPacket,
    codec::var_int::VarInt,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};
use papokin_util::version::JavaMinecraftVersion;

pub struct GameRuleEntry<'a> {
    pub game_rule_key: &'a str,
    pub value: &'a str,
}

#[java_packet(SET_GAME_RULE)]
pub struct SSetGameRule<'a> {
    pub entries: Vec<GameRuleEntry<'a>>,
}

impl<'a> ServerPacket<'a> for SSetGameRule<'a> {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let count = bytebuf.get_var_int()?.0;
        // count 直接来自网络，不可信。每个条目至少占 2 字节（两个空字符串
        // 的长度 VarInt 各 ≥1），先对照剩余包体长度校验，避免 with_capacity
        // 在任何读取失败之前因负数或巨额数量产生 panic/OOM。
        if count < 0 || count as usize > bytebuf.len() / 2 {
            return Err(ReadingError::Message(format!(
                "游戏规则条目数量超出包体长度允许的范围：{count}"
            )));
        }
        let count = count as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let game_rule_key = bytebuf.get_str_borrowed()?;
            let value = bytebuf.get_str_borrowed()?;
            entries.push(GameRuleEntry {
                game_rule_key,
                value,
            });
        }
        Ok(Self { entries })
    }
}

impl crate::ClientPacket for SSetGameRule<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_var_int(&VarInt(self.entries.len() as i32))?;
        for entry in &self.entries {
            write.write_string(entry.game_rule_key)?;
            write.write_string(entry.value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ser::NetworkWriteExt;

    fn read_result_with_count(count: i32) -> bool {
        let mut buf = Vec::new();
        buf.write_var_int(&VarInt(count)).unwrap();
        let version = JavaMinecraftVersion::V_1_21_11;
        let mut slice = buf.as_slice();
        SSetGameRule::read(&mut slice, &version).is_ok()
    }

    #[test]
    fn rejects_negative_entry_count() {
        // 负数 count 经 as usize 变为巨型值，未校验时会直接触发
        // Vec::with_capacity 的 capacity overflow panic（发生在
        // tick 线程，全服挂死）
        assert!(!read_result_with_count(-1));
    }

    #[test]
    fn rejects_count_beyond_packet_body() {
        // 2^31-1 个条目 ≈ 68.7 GB 分配，未校验时直接 OOM abort
        assert!(!read_result_with_count(i32::MAX));
    }

    #[test]
    fn accepts_well_formed_empty_entries() {
        // 合法边界：0 条目（客户端请求全量 gamerule 值的常规形态）
        assert!(read_result_with_count(0));
    }
}
