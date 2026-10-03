use crate::{
    ClientPacket, ServerPacket,
    ser::{NetworkReadSliceExt, NetworkWriteExt, ReadingError, WritingError},
};
use papokin_data::packet::clientbound::play::UPDATE_RECIPES;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(UPDATE_RECIPES)]
pub struct CUpdateRecipes<'a> {
    pub raw_data: &'a [u8],
}

impl<'a> CUpdateRecipes<'a> {
    #[must_use]
    pub const fn new(raw_data: &'a [u8]) -> Self {
        Self { raw_data }
    }

    /// 构建 1.21.2+ 的 `UPDATE_RECIPES` 负载：7 个配方属性集
    /// （烧炼/营火输入、锻造三槽）+ 切石机选项列表。静态表以物品名
    /// 存储（`papokin_data::recipe_property_sets`，线序逐字节取自
    /// Papo 1.21.11 实抓），此处经 `item_id_remap` 换算到目标版本的
    /// 冻结 id 空间；目标版本缺席的物品直接丢弃。
    ///
    /// 切石机段的线格式为 `输入 HolderSet + 选项 SlotDisplay`；
    /// `SelectableRecipe.recipe` 不上线（原版同步中恒为空，客户端
    /// 仅凭选项展示驱动切石机 UI 网格）。
    pub fn build_payload(version: JavaMinecraftVersion) -> Result<Vec<u8>, WritingError> {
        use papokin_data::item::Item;
        use papokin_data::item_id_remap::remap_item_id_for_version;
        use papokin_data::recipe_property_sets::{RECIPE_PROPERTY_SETS, STONECUTTER_OPTIONS};

        use crate::VarInt;
        use crate::java::client::play::recipe_book_add::{
            item_id_versioned, write_item_stack_slot_display,
        };

        let resolve = |name: &str| -> Option<i32> {
            let item = Item::from_registry_key(name)?;
            // 目标版本缺席的物品映射为 0（=air），从列表中剔除
            let id = remap_item_id_for_version(item.id, version) as i32;
            (id != 0).then_some(id)
        };

        let mut write = Vec::new();
        write.write_var_int(&VarInt(RECIPE_PROPERTY_SETS.len() as i32))?;
        for (name, items) in RECIPE_PROPERTY_SETS {
            write.write_string(name)?;
            let ids: Vec<i32> = items.iter().filter_map(|item| resolve(item)).collect();
            write.write_var_int(&VarInt(ids.len() as i32))?;
            for id in ids {
                write.write_var_int(&VarInt(id))?;
            }
        }

        // 输入或结果在目标版本缺席的条目整体跳过
        let mut entries: Vec<(Vec<i32>, &Item, u8)> = Vec::with_capacity(STONECUTTER_OPTIONS.len());
        for (inputs, result, count) in STONECUTTER_OPTIONS {
            let input_ids: Vec<i32> = inputs.iter().filter_map(|item| resolve(item)).collect();
            let Some(result_item) = Item::from_registry_key(result) else {
                continue;
            };
            if input_ids.is_empty() || item_id_versioned(result_item, version) == 0 {
                continue;
            }
            entries.push((input_ids, result_item, *count));
        }

        write.write_var_int(&VarInt(entries.len() as i32))?;
        for (input_ids, result_item, count) in entries {
            // HolderSet 直接列表：VarInt(n + 1) 后跟 n 个物品 id
            write.write_var_int(&VarInt(input_ids.len() as i32 + 1))?;
            for id in input_ids {
                write.write_var_int(&VarInt(id))?;
            }
            write_item_stack_slot_display(&mut write, result_item, count, version)?;
            // SelectableRecipe.recipe 不上线（恒空）
        }
        Ok(write)
    }
}

impl ClientPacket for CUpdateRecipes<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        write.write_slice(self.raw_data)?;
        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CUpdateRecipes<'a> {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let raw_data = bytebuf.read_remaining_slice_borrowed(usize::MAX)?;
        Ok(Self { raw_data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fnv1a(data: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in data {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    /// 构建出的 1.21.11 负载必须与 Papo 1.21.11 实抓逐字节一致
    /// （2819B，fnv1a-64 = fbc366f5824790cd）：线序、物品 id 重映射、
    /// 槽位显示类型换算任一出错都会改变字节。
    #[test]
    fn payload_matches_papo_1_21_11_capture() {
        let payload = CUpdateRecipes::build_payload(JavaMinecraftVersion::V_1_21_11).unwrap();
        assert_eq!(
            payload.len(),
            2819,
            "负载长度应等于 Papo 实抓（前 16 字节：{:02x?}）",
            &payload[..16.min(payload.len())]
        );
        assert_eq!(
            fnv1a(&payload),
            0xfbc3_66f5_8247_90cd,
            "负载 fnv1a 应与 Papo 实抓一致"
        );
    }
}
