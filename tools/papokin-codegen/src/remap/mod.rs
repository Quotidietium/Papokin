use papokin_nbt::compound::NbtCompound;
use proc_macro2::TokenStream;

use crate::version::JavaMinecraftVersion;

mod argument_type;
mod attribute;
mod block_entity_type;
mod block_state;
mod custom_stat;
mod data_component_type;
mod enchantment;
mod entity_id;
mod environment_attribute;
mod item_id;
mod menu_id;
mod painting_variant;
mod particle_id;
mod recipe_serializer;
mod slot_display;
mod sound_id;

/// 返回重映射构建函数及其对应输出文件名的列表。
#[allow(clippy::type_complexity)]
pub fn build() -> Vec<(fn() -> TokenStream, &'static str)> {
    vec![
        (argument_type::build, "argument_type_id_remap.rs"),
        (attribute::build, "attribute_id_remap.rs"),
        (block_entity_type::build, "block_entity_type_id_remap.rs"),
        (block_state::build, "block_state_remap.rs"),
        (custom_stat::build, "custom_stat_id_remap.rs"),
        (
            data_component_type::build,
            "data_component_type_id_remap.rs",
        ),
        (enchantment::build, "enchantment_id_remap.rs"),
        (entity_id::build, "entity_id_remap.rs"),
        (
            environment_attribute::build,
            "environment_attribute_id_remap.rs",
        ),
        (item_id::build, "item_id_remap.rs"),
        (menu_id::build, "menu_id_remap.rs"),
        (painting_variant::build, "painting_variant_id_remap.rs"),
        (particle_id::build, "particle_id_remap.rs"),
        (recipe_serializer::build, "recipe_serializer_id_remap.rs"),
        (slot_display::build, "slot_display_id_remap.rs"),
        (sound_id::build, "sound_id_remap.rs"),
    ]
}

#[macro_export]
macro_rules! remap_nodes {
    ($remapper:expr) => {{
        let node_1_7_6 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_7_6,
            value: "../../assets/viarewind/data/mappings-1.8to1.7.10.nbt",
            child: None,
        };
        let node_1_8 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_8,
            value: "../../assets/viarewind/data/mappings-1.9.4to1.8.nbt",
            child: Some(&node_1_7_6),
        };
        let node_1_9 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_9,
            value: "../../assets/viabackwards/data/mappings-1.10to1.9.4.nbt",
            child: Some(&node_1_8),
        };
        let node_1_10 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_10,
            value: "../../assets/viabackwards/data/mappings-1.11to1.10.nbt",
            child: Some(&node_1_9),
        };
        let node_1_11 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_11,
            value: "../../assets/viabackwards/data/mappings-1.12to1.11.nbt",
            child: Some(&node_1_10),
        };
        let node_1_12 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_12,
            value: "../../assets/viabackwards/data/mappings-1.13to1.12.nbt",
            child: Some(&node_1_11),
        };
        let node_1_13 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_13,
            value: "../../assets/viabackwards/data/mappings-1.13.2to1.13.nbt",
            child: Some(&node_1_12),
        };
        let node_1_13_2 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_13_2,
            value: "../../assets/viabackwards/data/mappings-1.14to1.13.2.nbt",
            child: Some(&node_1_13),
        };
        let node_1_14 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_14,
            value: "../../assets/viabackwards/data/mappings-1.15to1.14.nbt",
            child: Some(&node_1_13_2),
        };
        let node_1_15 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_15,
            value: "../../assets/viabackwards/data/mappings-1.16to1.15.nbt",
            child: Some(&node_1_14),
        };
        let node_1_16 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_16,
            value: "../../assets/viabackwards/data/mappings-1.16.2to1.16.nbt",
            child: Some(&node_1_15),
        };
        let node_1_16_2 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_16_2,
            value: "../../assets/viabackwards/data/mappings-1.17to1.16.2.nbt",
            child: Some(&node_1_16),
        };
        let node_1_17 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_17,
            value: "../../assets/viabackwards/data/mappings-1.18to1.17.nbt",
            child: Some(&node_1_16_2),
        };
        let node_1_18 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_18,
            value: "../../assets/viabackwards/data/mappings-1.19to1.18.nbt",
            child: Some(&node_1_17),
        };
        let node_1_19 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_19,
            value: "../../assets/viabackwards/data/mappings-1.19.3to1.19.nbt",
            child: Some(&node_1_18),
        };
        let node_1_19_3 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_19_3,
            value: "../../assets/viabackwards/data/mappings-1.19.4to1.19.3.nbt",
            child: Some(&node_1_19),
        };
        let node_1_19_4 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_19_4,
            value: "../../assets/viabackwards/data/mappings-1.20to1.19.4.nbt",
            child: Some(&node_1_19_3),
        };
        let node_1_20 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_20,
            value: "../../assets/viabackwards/data/mappings-1.20.2to1.20.nbt",
            child: Some(&node_1_19_4),
        };
        let node_1_20_2 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_20_2,
            value: "../../assets/viabackwards/data/mappings-1.20.3to1.20.2.nbt",
            child: Some(&node_1_20),
        };
        let node_1_20_3 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_20_3,
            value: "../../assets/viabackwards/data/mappings-1.20.5to1.20.3.nbt",
            child: Some(&node_1_20_2),
        };
        let node_1_20_5 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_20_5,
            value: "../../assets/viabackwards/data/mappings-1.21to1.20.5.nbt",
            child: Some(&node_1_20_3),
        };
        let node_1_21 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21,
            value: "../../assets/viabackwards/data/mappings-1.21.2to1.21.nbt",
            child: Some(&node_1_20_5),
        };
        let node_1_21_2 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_2,
            value: "../../assets/viabackwards/data/mappings-1.21.4to1.21.2.nbt",
            child: Some(&node_1_21),
        };
        let node_1_21_4 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_4,
            value: "../../assets/viabackwards/data/mappings-1.21.5to1.21.4.nbt",
            child: Some(&node_1_21_2),
        };
        let node_1_21_5 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_5,
            value: "../../assets/viabackwards/data/mappings-1.21.6to1.21.5.nbt",
            child: Some(&node_1_21_4),
        };
        let node_1_21_6 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_6,
            value: "../../assets/viabackwards/data/mappings-1.21.7to1.21.6.nbt",
            child: Some(&node_1_21_5),
        };
        let node_1_21_7 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_7,
            value: "../../assets/viabackwards/data/mappings-1.21.9to1.21.7.nbt",
            child: Some(&node_1_21_6),
        };
        let node_1_21_9 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_9,
            value: "../../assets/viabackwards/data/mappings-1.21.11to1.21.9.nbt",
            child: Some(&node_1_21_7),
        };
        let node_1_21_11 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_1_21_11,
            value: "../../assets/viabackwards/data/mappings-26.1to1.21.11.nbt",
            child: Some(&node_1_21_9),
        };
        let node_26_1 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_26_1,
            value: "../../assets/viabackwards/data/mappings-26.2to26.1.nbt",
            child: Some(&node_1_21_11),
        };
        let node_26_2 = $crate::remap::MappingNode {
            version: $crate::version::JavaMinecraftVersion::V_26_2,
            value: "../../assets/viabackwards/data/mappings-26.3to26.2.nbt",
            child: Some(&node_26_1),
        };
        $remapper.process(&node_26_2)
    }};
}

/// ViaVersion 映射文件链表中的一个节点，每个节点描述 ID 如何变化
/// 在相邻的 Minecraft 版本之间。
pub struct MappingNode<'a, P> {
    /// 此节点所表示的 Minecraft 版本。
    pub version: JavaMinecraftVersion,
    /// 此版本跨越所用的 ViaVersion NBT 映射文件的路径（或数据）。
    pub value: P,
    /// 链中的上一个版本节点；如果这是受支持的最早版本，则为 `None`。
    pub child: Option<&'a Self>,
}

/// 驱动 [`MappingNode`] 链的递归处理，组合各中间映射
/// 转换为按版本的翻译表。
pub struct Remapper<P, R> {
    /// 所有旧版映射都要向其转换的目标（最新）版本。
    pub version: JavaMinecraftVersion,
    /// 将当前版本映射与子映射组合成一个复合映射。
    pub remapper: fn(&R, &R) -> R,
    /// 将 [`MappingNode`] 中存储的原始路径/数据 `P` 转换为映射类型 `R`。
    pub serializer: fn(&P) -> R,
}

impl<P, R> Remapper<P, R> {
    /// 递归处理 [`MappingNode`] 链，并返回 `(version, mapping)` 对的列表。
    ///
    /// # Returns
    /// 一个 `Vec`，其中每一项包含一个 [`JavaMinecraftVersion`] 及其组合映射相对于
    /// 到 `self.version`。
    pub fn process(&self, mappings: &MappingNode<'_, P>) -> Vec<(JavaMinecraftVersion, R)> {
        let current_mapping = (self.serializer)(&mappings.value);
        let mut remap = if let Some(child) = mappings.child {
            let mut res = self.process(child);
            for (_, remap) in &mut res {
                let new_mapping = (self.remapper)(&current_mapping, remap);
                *remap = new_mapping;
            }
            res
        } else {
            Vec::new()
        };
        remap.push((mappings.version, current_mapping));
        remap
    }
}

/// 一个已解码的 ViaVersion ID 映射，附带正向转换表。
pub struct ParsedMappings {
    /// 映射（较新）版本命名空间中的 ID 数量。
    pub mapped_size: usize,
    /// 正向映射：索引为旧 ID，值为新 ID（`-1` 表示未映射）。
    pub forward: Vec<i32>,
}

impl ParsedMappings {
    /// 读取并解析 ViaVersion 的 NBT 映射文件，提取指定名称的部分。
    ///
    /// # Arguments
    /// - `path` – `.nbt` 映射文件的路径。
    /// - `section` – 要提取的复合段名称（例如 `"blockstates"`、`"items"`）。
    ///
    /// # Returns
    /// 对应区段存在时返回 `Some(ParsedMappings)`，区段缺失时返回 `None`。
    pub fn parse_mapping_file(path: &str, section: &str) -> Option<Self> {
        use papokin_nbt::Nbt;
        use papokin_nbt::deserializer::NbtReadHelperJava;
        use std::fs;
        use std::io::Cursor;

        let bytes = fs::read(path).unwrap_or_else(|_| panic!("读取 {path} 失败"));
        let mut reader = NbtReadHelperJava::new(Cursor::new(bytes));
        let nbt = Nbt::read(&mut reader).unwrap_or_else(|_| panic!("解析 {path} 处的 NBT 失败"));

        let mappings = nbt.root_tag.get_compound(section)?;
        // .unwrap_or_else(|| panic!("Missing `{section}` compound in {path}"));

        Some(Self::parse_mappings(mappings, path, section))
    }

    /// 将 ViaVersion 映射复合标签解码为正向 ID 转换表。
    fn parse_mappings(mappings: &NbtCompound, path: &str, section: &str) -> Self {
        let mapped_size = mappings
            .get_int("mappedSize")
            .unwrap_or_else(|| panic!("{path} 中缺少 `{section}.mappedSize`"));
        let strategy = mappings
            .get_byte("id")
            .unwrap_or_else(|| panic!("{path} 中缺少 `{section}.id`"));

        let forward = match strategy {
            // 直接
            0 => {
                if let Some(val) = mappings.get_int_array("val") {
                    val.to_vec()
                } else if let Some(val_bytes) = mappings.get_byte_array("val") {
                    let size = mappings.get_int("size").unwrap_or(mapped_size) as usize;
                    let bytes: &[u8] = unsafe {
                        std::slice::from_raw_parts(val_bytes.as_ptr().cast::<u8>(), val_bytes.len())
                    };
                    let mut cursor = std::io::Cursor::new(bytes);
                    let mut values = Vec::with_capacity(size);
                    let mut prev = 0i32;
                    for _ in 0..size {
                        prev += Self::read_zigzag_var_int(&mut cursor).unwrap_or(0);
                        values.push(prev);
                    }
                    values
                } else {
                    panic!("{path} 中的 direct 映射缺少 `{section}.val`");
                }
            }
            // 位移量
            1 => {
                let (shifts_at, shifts_to) = if let (Some(at), Some(to)) =
                    (mappings.get_int_array("at"), mappings.get_int_array("to"))
                {
                    (at.to_vec(), to.to_vec())
                } else if let Some(val_bytes) = mappings.get_byte_array("val") {
                    Self::read_at_value_pairs(val_bytes)
                } else {
                    panic!("{path} 中的 shift 映射缺少 `{section}.at`/`to` 或 `{section}.val`");
                };

                let size = mappings
                    .get_int("size")
                    .unwrap_or_else(|| panic!("{path} 中的 shift 映射缺少 `{section}.size`"))
                    as usize;

                assert_eq!(
                    shifts_at.len(),
                    shifts_to.len(),
                    "Shift mapping length mismatch in {path}"
                );

                let mut result = vec![-1; size];

                if !shifts_at.is_empty() && shifts_at[0] != 0 {
                    for id in 0..shifts_at[0] {
                        result[id as usize] = id;
                    }
                }

                for (index, from) in shifts_at.iter().enumerate() {
                    let to = if index + 1 == shifts_at.len() {
                        size as i32
                    } else {
                        shifts_at[index + 1]
                    };
                    for (mapped_id, id) in (shifts_to[index]..).zip(*from..to) {
                        result[id as usize] = mapped_id;
                    }
                }

                result
            }
            // 变更
            2 => {
                let (changes_at, values) = if let (Some(at), Some(val)) =
                    (mappings.get_int_array("at"), mappings.get_int_array("val"))
                {
                    (at.to_vec(), val.to_vec())
                } else if let Some(val_bytes) = mappings.get_byte_array("val") {
                    Self::read_at_value_pairs(val_bytes)
                } else {
                    panic!("{path} 中的 change 映射缺少 `{section}.at`/`val`");
                };

                let size = mappings
                    .get_int("size")
                    .unwrap_or_else(|| panic!("{path} 中的 change 映射缺少 `{section}.size`"))
                    as usize;
                let fill_between = mappings.get("nofill").is_none();

                assert_eq!(
                    changes_at.len(),
                    values.len(),
                    "Change mapping length mismatch in {path}"
                );

                let mut result = vec![-1; size];
                let mut next_unhandled_id = 0;

                for (index, changed_id) in changes_at.iter().enumerate() {
                    if fill_between {
                        for id in next_unhandled_id..*changed_id {
                            result[id as usize] = id;
                        }
                        next_unhandled_id = changed_id + 1;
                    }
                    result[*changed_id as usize] = values[index];
                }

                if fill_between {
                    for id in next_unhandled_id..size as i32 {
                        result[id as usize] = id;
                    }
                }

                result
            }
            // 恒等
            3 => {
                let size = mappings
                    .get_int("size")
                    .unwrap_or_else(|| panic!("{path} 中的 identity 映射缺少 `{section}.size`"))
                    as usize;
                (0..size as i32).collect::<Vec<_>>()
            }
            _ => panic!("{path} 中未知的 {section} 映射策略 {strategy}"),
        };

        Self {
            mapped_size: mapped_size as usize,
            forward,
        }
    }

    fn read_var_int(cursor: &mut std::io::Cursor<&[u8]>) -> Option<i32> {
        use std::io::Read;
        let mut num_read = 0;
        let mut result = 0i32;
        loop {
            let mut byte = [0u8; 1];
            if cursor.read_exact(&mut byte).is_err() {
                return None;
            }
            let b = byte[0];
            let value = (b & 0b0111_1111) as i32;
            result |= value << (7 * num_read);
            num_read += 1;
            if num_read > 5 {
                return None;
            }
            if (b & 0b1000_0000) == 0 {
                break;
            }
        }
        Some(result)
    }

    fn read_zigzag_var_int(cursor: &mut std::io::Cursor<&[u8]>) -> Option<i32> {
        let value = Self::read_var_int(cursor)?;
        let unsigned = value as u32;
        Some(((unsigned >> 1) as i32) ^ (-((unsigned & 1) as i32)))
    }

    fn read_at_value_pairs(val_bytes: &[i8]) -> (Vec<i32>, Vec<i32>) {
        let bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(val_bytes.as_ptr().cast::<u8>(), val_bytes.len()) };
        let mut cursor = std::io::Cursor::new(bytes);
        let mut at = Vec::new();
        let mut values = Vec::new();
        let mut prev_at = -1i32;
        let mut prev_val = 0i32;
        while let Some(diff_at) = Self::read_var_int(&mut cursor) {
            let diff_val = Self::read_zigzag_var_int(&mut cursor).unwrap_or(0);
            prev_at = prev_at + 1 + diff_at;
            prev_val += diff_val;
            at.push(prev_at);
            values.push(prev_val);
        }
        (at, values)
    }

    /// 将正向映射反转成反向查找表，其中索引为新 ID，值为
    /// 是对应的旧 ID。未映射的条目默认取其自身索引转换为 `u16` 的值。
    ///
    /// # Arguments
    /// - `name` – 用于 panic 消息的描述性名称，便于诊断。
    ///
    /// # Returns
    /// 一个长度为 `self.mapped_size` 的 `Vec<u16>`，将新 ID 映射回旧 ID。
    pub fn _invert_with_default_to_u16(&self, name: &str) -> Vec<u16> {
        let mut inverse = vec![0u16; self.mapped_size];
        let mut seen = vec![false; self.mapped_size];

        for (old_id, mapped_id) in self.forward.iter().enumerate() {
            let Ok(mapped_id) = usize::try_from(*mapped_id) else {
                continue;
            };
            if mapped_id >= self.mapped_size || seen[mapped_id] {
                continue;
            }

            let old_u16 = u16::try_from(old_id)
                .unwrap_or_else(|_| panic!("{name}: id {old_id} 无法放入 u16"));
            inverse[mapped_id] = old_u16;
            seen[mapped_id] = true;
        }

        for (mapped_id, mapped_to) in inverse.iter_mut().enumerate() {
            if !seen[mapped_id] {
                *mapped_to = u16::try_from(mapped_id)
                    .unwrap_or_else(|_| panic!("{name}: id {mapped_id} 无法放入 u16"));
            }
        }

        inverse
    }

    /// 将正向映射直接转换为 u16 表。
    /// 与已处于新→旧方向的 ViaBackwards 映射配合使用。
    pub fn to_u16(&self, name: &str) -> Vec<u16> {
        self.forward
            .iter()
            .map(|&id| {
                if id < 0 {
                    0 // 未映射 → 空气
                } else if id > 0xFFFF {
                    // 针对 1.13 之前的映射，其中 itemId 按 (id << 16) | data 打包
                    u16::try_from(id >> 16)
                        .unwrap_or_else(|_| panic!("{name}: id {id} 无法放入 u16"))
                } else {
                    u16::try_from(id).unwrap_or_else(|_| panic!("{name}: id {id} 无法放入 u16"))
                }
            })
            .collect()
    }

    /// 将正向映射直接转换为 u32 表。
    /// 与已处于新→旧方向的 ViaBackwards 映射配合使用。
    pub fn to_u32(&self, name: &str) -> Vec<u32> {
        self.forward
            .iter()
            .map(|&id| {
                if id < 0 {
                    0
                } else {
                    u32::try_from(id).unwrap_or_else(|_| panic!("{name}: id {id} 无法放入 u32"))
                }
            })
            .collect()
    }
}

///返回共享相同协议数据映射的所有 JavaMinecraftVersion 变体。
#[must_use]
pub fn version_patterns(ver: JavaMinecraftVersion) -> Vec<JavaMinecraftVersion> {
    match ver {
        JavaMinecraftVersion::V_1_7_6 => {
            vec![JavaMinecraftVersion::V_1_7_2, JavaMinecraftVersion::V_1_7_6]
        }
        JavaMinecraftVersion::V_1_8 => vec![JavaMinecraftVersion::V_1_8],
        JavaMinecraftVersion::V_1_9 => vec![
            JavaMinecraftVersion::V_1_9,
            JavaMinecraftVersion::V_1_9_1,
            JavaMinecraftVersion::V_1_9_2,
            JavaMinecraftVersion::V_1_9_3,
        ],
        JavaMinecraftVersion::V_1_10 => vec![JavaMinecraftVersion::V_1_10],
        JavaMinecraftVersion::V_1_11 => {
            vec![JavaMinecraftVersion::V_1_11, JavaMinecraftVersion::V_1_11_1]
        }
        JavaMinecraftVersion::V_1_12 => vec![
            JavaMinecraftVersion::V_1_12,
            JavaMinecraftVersion::V_1_12_1,
            JavaMinecraftVersion::V_1_12_2,
        ],
        JavaMinecraftVersion::V_1_13 => {
            vec![JavaMinecraftVersion::V_1_13, JavaMinecraftVersion::V_1_13_1]
        }
        JavaMinecraftVersion::V_1_13_2 => vec![JavaMinecraftVersion::V_1_13_2],
        JavaMinecraftVersion::V_1_14 => vec![
            JavaMinecraftVersion::V_1_14,
            JavaMinecraftVersion::V_1_14_1,
            JavaMinecraftVersion::V_1_14_2,
            JavaMinecraftVersion::V_1_14_3,
            JavaMinecraftVersion::V_1_14_4,
        ],
        JavaMinecraftVersion::V_1_15 => vec![
            JavaMinecraftVersion::V_1_15,
            JavaMinecraftVersion::V_1_15_1,
            JavaMinecraftVersion::V_1_15_2,
        ],
        JavaMinecraftVersion::V_1_16 => {
            vec![JavaMinecraftVersion::V_1_16, JavaMinecraftVersion::V_1_16_1]
        }
        JavaMinecraftVersion::V_1_16_2 => vec![
            JavaMinecraftVersion::V_1_16_2,
            JavaMinecraftVersion::V_1_16_3,
            JavaMinecraftVersion::V_1_16_4,
        ],
        JavaMinecraftVersion::V_1_17 => {
            vec![JavaMinecraftVersion::V_1_17, JavaMinecraftVersion::V_1_17_1]
        }
        JavaMinecraftVersion::V_1_18 => {
            vec![JavaMinecraftVersion::V_1_18, JavaMinecraftVersion::V_1_18_2]
        }
        JavaMinecraftVersion::V_1_19 => {
            vec![JavaMinecraftVersion::V_1_19, JavaMinecraftVersion::V_1_19_1]
        }
        JavaMinecraftVersion::V_1_19_3 => vec![JavaMinecraftVersion::V_1_19_3],
        JavaMinecraftVersion::V_1_19_4 => vec![JavaMinecraftVersion::V_1_19_4],
        JavaMinecraftVersion::V_1_20 => vec![JavaMinecraftVersion::V_1_20],
        JavaMinecraftVersion::V_1_20_2 => vec![JavaMinecraftVersion::V_1_20_2],
        JavaMinecraftVersion::V_1_20_3 => vec![JavaMinecraftVersion::V_1_20_3],
        JavaMinecraftVersion::V_1_20_5 => vec![JavaMinecraftVersion::V_1_20_5],
        JavaMinecraftVersion::V_1_21 => vec![JavaMinecraftVersion::V_1_21],
        JavaMinecraftVersion::V_1_21_2 => vec![JavaMinecraftVersion::V_1_21_2],
        JavaMinecraftVersion::V_1_21_4 => vec![JavaMinecraftVersion::V_1_21_4],
        JavaMinecraftVersion::V_1_21_5 => vec![JavaMinecraftVersion::V_1_21_5],
        JavaMinecraftVersion::V_1_21_6 => vec![JavaMinecraftVersion::V_1_21_6],
        JavaMinecraftVersion::V_1_21_7 => vec![JavaMinecraftVersion::V_1_21_7],
        JavaMinecraftVersion::V_1_21_9 => vec![JavaMinecraftVersion::V_1_21_9],
        JavaMinecraftVersion::V_1_21_11 => vec![JavaMinecraftVersion::V_1_21_11],
        JavaMinecraftVersion::V_26_1 => vec![JavaMinecraftVersion::V_26_1],
        JavaMinecraftVersion::V_26_2 => vec![JavaMinecraftVersion::V_26_2],
        JavaMinecraftVersion::V_26_3 => vec![JavaMinecraftVersion::V_26_3],
        _ => vec![ver],
    }
}
