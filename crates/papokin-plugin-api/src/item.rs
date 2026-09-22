pub use crate::generated::item::Item;
use crate::wit::papokin::plugin::item_stack::ItemStack;

/// 用于将类型转换为有效的 Minecraft 物品注册表键的 trait（例如 `"minecraft:diamond"`）。
pub trait IntoItemKey {
    /// 返回带命名空间的物品注册表键。
    fn into_item_key(self) -> String;
}

impl IntoItemKey for Item {
    fn into_item_key(self) -> String {
        self.resource_location().to_string()
    }
}

impl IntoItemKey for &Item {
    fn into_item_key(self) -> String {
        self.resource_location().to_string()
    }
}

impl IntoItemKey for &str {
    fn into_item_key(self) -> String {
        if self.contains(':') {
            self.to_string()
        } else {
            format!("minecraft:{self}")
        }
    }
}

impl IntoItemKey for String {
    fn into_item_key(self) -> String {
        if self.contains(':') {
            self
        } else {
            format!("minecraft:{self}")
        }
    }
}

impl IntoItemKey for &String {
    fn into_item_key(self) -> String {
        self.as_str().into_item_key()
    }
}

/// 在 `ItemStack` 上提供类型化辅助构造器与工具的扩展 trait。
pub trait ItemStackExt {
    /// 从任意合法物品键（如 `Item::Diamond`、`"minecraft:diamond"` 或 `"custom:item"`）创建新的 `ItemStack`。
    #[must_use]
    fn of(item: impl IntoItemKey, count: u8) -> Self;

    /// 若此物品堆对应已知的原版物品，则返回对应的 `Item` 枚举，
    /// 若为自定义、模组或较新版本的未知物品，则返回 `None`。
    #[must_use]
    fn get_item(&self) -> Option<Item>;

    /// 返回此物品是否匹配指定的 `Item` 枚举类型。
    #[must_use]
    fn is_item(&self, item: Item) -> bool;

    /// 检查该物品堆是否匹配给定物品（`Item` 枚举或字符串标识符均可）。
    #[must_use]
    fn matches_item(&self, item: impl IntoItemKey) -> bool;
}

impl ItemStackExt for ItemStack {
    fn of(item: impl IntoItemKey, count: u8) -> Self {
        Self::new(&item.into_item_key(), count)
    }

    fn get_item(&self) -> Option<Item> {
        Item::from_registry_key(&self.get_registry_key())
    }

    fn is_item(&self, item: Item) -> bool {
        self.get_item() == Some(item)
    }

    fn matches_item(&self, item: impl IntoItemKey) -> bool {
        let expected = item.into_item_key();
        let actual = self.get_registry_key();
        let expected_clean = expected.strip_prefix("minecraft:").unwrap_or(&expected);
        let actual_clean = actual.strip_prefix("minecraft:").unwrap_or(&actual);
        expected_clean == actual_clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_to_resource_location() {
        assert_eq!(Item::Diamond.into_item_key(), "minecraft:diamond");
        assert_eq!(
            Item::NetheriteSword.into_item_key(),
            "minecraft:netherite_sword"
        );
        assert_eq!(Item::AcaciaBoat.into_item_key(), "minecraft:acacia_boat");
        assert_eq!(
            Item::AllaySpawnEgg.into_item_key(),
            "minecraft:allay_spawn_egg"
        );
        assert_eq!(Item::Tnt.into_item_key(), "minecraft:tnt");
    }

    #[test]
    fn string_to_resource_location() {
        assert_eq!("diamond".into_item_key(), "minecraft:diamond");
        assert_eq!("minecraft:diamond".into_item_key(), "minecraft:diamond");
        assert_eq!("custom:laser_gun".into_item_key(), "custom:laser_gun");
    }

    #[test]
    fn item_parsing() {
        assert_eq!(Item::from_name("diamond"), Some(Item::Diamond));
        assert_eq!(Item::from_name("minecraft:diamond"), Some(Item::Diamond));
        assert_eq!(
            Item::from_name("netherite_sword"),
            Some(Item::NetheriteSword)
        );
        assert_eq!(Item::from_name("custom:magic_wand"), None);
    }
}
