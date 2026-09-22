pub use crate::generated::block::BlockType;
use crate::wit::papokin::plugin::world::{Block, BlockState};

/// 用于将类型转换为有效的 Minecraft 方块注册表键的 trait（例如 `"minecraft:stone"`）。
pub trait IntoBlockKey {
    /// 返回带命名空间的方块注册表键。
    fn into_block_key(self) -> String;
}

impl IntoBlockKey for BlockType {
    fn into_block_key(self) -> String {
        self.resource_location().to_string()
    }
}

impl IntoBlockKey for &BlockType {
    fn into_block_key(self) -> String {
        self.resource_location().to_string()
    }
}

impl IntoBlockKey for &str {
    fn into_block_key(self) -> String {
        if self.contains(':') {
            self.to_string()
        } else {
            format!("minecraft:{self}")
        }
    }
}

impl IntoBlockKey for String {
    fn into_block_key(self) -> String {
        if self.contains(':') {
            self
        } else {
            format!("minecraft:{self}")
        }
    }
}

impl IntoBlockKey for &String {
    fn into_block_key(self) -> String {
        self.as_str().into_block_key()
    }
}

/// 在 `Block` 上提供类型化辅助方法的扩展 trait。
pub trait BlockTypeExt {
    /// 按任意合法方块键（如 `BlockType::Stone`、`"minecraft:stone"` 或 `"custom:block"`）获取方块定义。
    #[must_use]
    fn of(key: impl IntoBlockKey) -> Option<Block>;

    /// 若此方块对应已知的原版方块，则返回对应的 `BlockType`，
    /// 若为自定义、模组或较新版本的未知方块，则返回 `None`。
    #[must_use]
    fn get_block_type(&self) -> Option<BlockType>;

    /// 检查该方块是否匹配给定的 `BlockType`。
    #[must_use]
    fn is_block_type(&self, block_type: BlockType) -> bool;

    /// 检查该方块是否匹配给定的方块键。
    #[must_use]
    fn matches_block(&self, key: impl IntoBlockKey) -> bool;
}

impl BlockTypeExt for Block {
    fn of(key: impl IntoBlockKey) -> Option<Block> {
        Block::from_name(&key.into_block_key())
    }

    fn get_block_type(&self) -> Option<BlockType> {
        BlockType::from_registry_key(&self.name)
    }

    fn is_block_type(&self, block_type: BlockType) -> bool {
        self.get_block_type() == Some(block_type)
    }

    fn matches_block(&self, key: impl IntoBlockKey) -> bool {
        let expected = key.into_block_key();
        let expected_clean = expected.strip_prefix("minecraft:").unwrap_or(&expected);
        let actual_clean = self.name.strip_prefix("minecraft:").unwrap_or(&self.name);
        expected_clean == actual_clean
    }
}

/// 在 `BlockState` 上提供类型化辅助方法的扩展 trait。
pub trait BlockStateTypeExt {
    /// 若此方块状态对应已知的原版方块，则返回对应的 `BlockType`，
    /// 若为自定义、模组或较新版本的未知方块，则返回 `None`。
    #[must_use]
    fn get_block_type(&self) -> Option<BlockType>;

    /// 检查该方块状态是否匹配给定的 `BlockType`。
    #[must_use]
    fn is_block_type(&self, block_type: BlockType) -> bool;

    /// 检查该方块状态是否匹配给定的方块键。
    #[must_use]
    fn matches_block(&self, key: impl IntoBlockKey) -> bool;
}

impl BlockStateTypeExt for BlockState {
    fn get_block_type(&self) -> Option<BlockType> {
        BlockType::from_registry_key(&self.block_name)
    }

    fn is_block_type(&self, block_type: BlockType) -> bool {
        self.get_block_type() == Some(block_type)
    }

    fn matches_block(&self, key: impl IntoBlockKey) -> bool {
        let expected = key.into_block_key();
        let expected_clean = expected.strip_prefix("minecraft:").unwrap_or(&expected);
        let actual_clean = self
            .block_name
            .strip_prefix("minecraft:")
            .unwrap_or(&self.block_name);
        expected_clean == actual_clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_to_resource_location() {
        assert_eq!(BlockType::Stone.into_block_key(), "minecraft:stone");
        assert_eq!(
            BlockType::DiamondBlock.into_block_key(),
            "minecraft:diamond_block"
        );
        assert_eq!(BlockType::OakLog.into_block_key(), "minecraft:oak_log");
        assert_eq!(BlockType::Obsidian.into_block_key(), "minecraft:obsidian");
    }

    #[test]
    fn string_to_resource_location() {
        assert_eq!("stone".into_block_key(), "minecraft:stone");
        assert_eq!("minecraft:stone".into_block_key(), "minecraft:stone");
        assert_eq!("custom:ruby_block".into_block_key(), "custom:ruby_block");
    }

    #[test]
    fn block_parsing() {
        assert_eq!(BlockType::from_name("stone"), Some(BlockType::Stone));
        assert_eq!(
            BlockType::from_name("minecraft:stone"),
            Some(BlockType::Stone)
        );
        assert_eq!(
            BlockType::from_name("diamond_block"),
            Some(BlockType::DiamondBlock)
        );
        assert_eq!(BlockType::from_name("custom:ruby_block"), None);
    }
}
