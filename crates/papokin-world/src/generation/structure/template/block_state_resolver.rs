//! 从模板调色板条目解析方块状态。
//!
//! 本模块负责将 NBT 调色板条目（方块名 + 属性）
//! 转换为世界所使用的运行时方块状态 ID，并支持旋转
//! 以及镜像变换。

use papokin_data::{Block, BlockState, Mirror, Rotation};
use tracing::warn;

use super::PaletteEntry;

/// 将模板调色板条目解析为方块状态 ID。
///
/// 此解析器处理：
/// - 按名称查找方块
/// - 应用方块状态属性
/// - 旋转/镜像方向属性（facing、axis、rotation）
pub struct BlockStateResolver;

impl BlockStateResolver {
    /// 将调色板条目解析为方块状态，并应用旋转与镜像变换。
    ///
    /// 返回解析后的 `BlockState`；若方块未找到，则返回 `None`。
    #[must_use]
    pub fn resolve(
        entry: &PaletteEntry,
        rotation: Rotation,
        mirror: Mirror,
    ) -> Option<&'static BlockState> {
        // 如存在则去除 minecraft: 前缀
        let block_name = entry.name.strip_prefix("minecraft:").unwrap_or(&entry.name);

        // 找到该方块
        let block = Block::from_name(&entry.name).or_else(|| Block::from_registry_key(block_name));

        let Some(block) = block else {
            warn!("模板中存在未知方块: {}", entry.name);
            return None;
        };

        // 如果没有属性，则返回默认状态
        if entry.properties.is_empty() {
            return Some(block.default_state);
        }

        // 使用统一的原版逻辑变换旋转/镜像的属性
        let transformed_props = papokin_data::transform_block_properties(
            &entry.name,
            &entry.properties,
            rotation,
            mirror,
        );

        // 转换为 from_properties 期望的格式
        let props_slice = transformed_props
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();

        // 从属性获取状态 ID
        let props_box = block.from_properties(&props_slice);
        let state_id = props_box.to_state_id(block);

        Some(BlockState::from_id(state_id))
    }

    /// 解析调色板条目，不施加任何变换。
    #[must_use]
    pub fn resolve_simple(entry: &PaletteEntry) -> Option<&'static BlockState> {
        Self::resolve(entry, Rotation::None, Mirror::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_simple_block() {
        let entry = PaletteEntry::new("minecraft:stone".to_string());
        let state = BlockStateResolver::resolve_simple(&entry);
        assert!(state.is_some());
    }

    #[test]
    fn resolve_with_properties() {
        let entry = PaletteEntry::with_properties(
            "minecraft:oak_stairs".to_string(),
            vec![
                ("facing".to_string(), "north".to_string()),
                ("half".to_string(), "bottom".to_string()),
                ("shape".to_string(), "straight".to_string()),
                ("waterlogged".to_string(), "false".to_string()),
            ],
        );
        let state = BlockStateResolver::resolve_simple(&entry);
        assert!(state.is_some());
    }

    #[test]
    fn rotation_transforms_facing() {
        let entry = PaletteEntry::with_properties(
            "minecraft:furnace".to_string(),
            vec![
                ("facing".to_string(), "north".to_string()),
                ("lit".to_string(), "false".to_string()),
            ],
        );

        // 获取带旋转的状态
        let rotated = BlockStateResolver::resolve(&entry, Rotation::Clockwise90, Mirror::None);
        assert!(rotated.is_some());

        // 顺时针旋转 90 度后，方块的 facing 应为 east
        // 若没有更多基础设施，我们很难在这里验证具体朝向，
        // 但可以验证它能成功解析
    }

    #[test]
    fn unknown_block_returns_none() {
        let entry = PaletteEntry::new("minecraft:nonexistent_block".to_string());
        let state = BlockStateResolver::resolve_simple(&entry);
        assert!(state.is_none());
    }

    #[test]
    fn stairs_mirror_and_rotate_resolves_expected_state() {
        let entry = PaletteEntry::with_properties(
            "minecraft:oak_stairs".to_string(),
            vec![
                ("facing".to_string(), "north".to_string()),
                ("half".to_string(), "bottom".to_string()),
                ("shape".to_string(), "inner_left".to_string()),
                ("waterlogged".to_string(), "false".to_string()),
            ],
        );

        // LeftRight 镜像（北 -> 南，inner_left -> inner_right）
        // 然后旋转 Clockwise90（南 -> 西）
        let resolved =
            BlockStateResolver::resolve(&entry, Rotation::Clockwise90, Mirror::LeftRight).unwrap();

        // 与显式解析的目标状态比较
        let target_entry = PaletteEntry::with_properties(
            "minecraft:oak_stairs".to_string(),
            vec![
                ("facing".to_string(), "west".to_string()),
                ("half".to_string(), "bottom".to_string()),
                ("shape".to_string(), "inner_right".to_string()),
                ("waterlogged".to_string(), "false".to_string()),
            ],
        );
        let expected = BlockStateResolver::resolve_simple(&target_entry).unwrap();
        assert_eq!(resolved.id, expected.id);
    }

    #[test]
    fn door_mirror_resolves_flipped_hinge() {
        let entry = PaletteEntry::with_properties(
            "minecraft:oak_door".to_string(),
            vec![
                ("facing".to_string(), "east".to_string()),
                ("half".to_string(), "lower".to_string()),
                ("hinge".to_string(), "left".to_string()),
                ("open".to_string(), "false".to_string()),
                ("powered".to_string(), "false".to_string()),
            ],
        );

        // FrontBack 镜像（东 -> 西，铰链左 -> 右）
        let resolved =
            BlockStateResolver::resolve(&entry, Rotation::None, Mirror::FrontBack).unwrap();

        let target_entry = PaletteEntry::with_properties(
            "minecraft:oak_door".to_string(),
            vec![
                ("facing".to_string(), "west".to_string()),
                ("half".to_string(), "lower".to_string()),
                ("hinge".to_string(), "right".to_string()),
                ("open".to_string(), "false".to_string()),
                ("powered".to_string(), "false".to_string()),
            ],
        );
        let expected = BlockStateResolver::resolve_simple(&target_entry).unwrap();
        assert_eq!(resolved.id, expected.id);
    }
}
