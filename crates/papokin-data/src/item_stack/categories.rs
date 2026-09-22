use crate::tag;
use crate::tag::Taggable;

use crate::item_stack::ItemStack;

impl ItemStack {
    #[inline]
    #[must_use]
    pub fn is_sword(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_SWORDS)
    }

    #[inline]
    #[must_use]
    pub fn is_helmet(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_HEAD_ARMOR)
    }

    #[inline]
    #[must_use]
    pub fn is_skull(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_SKULLS)
    }

    #[inline]
    #[must_use]
    pub fn is_chestplate(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_CHEST_ARMOR)
    }

    #[inline]
    #[must_use]
    pub fn is_leggings(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_LEG_ARMOR)
    }

    #[inline]
    #[must_use]
    pub fn is_boots(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_FOOT_ARMOR)
    }

    /// 如果物品属于 `#minecraft:enchantable/armor` 则为 `true`（选择盔甲耐久公式）。
    #[inline]
    #[must_use]
    pub fn is_armor(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_ENCHANTABLE_ARMOR)
    }

    /// 仅供测试的谓词：识别 2 点耐久度的工具（斧/镐/锹/锄）。
    /// 在实际游戏中，耐久消耗由 `Weapon` 组件以数据驱动。
    /// 这些辅助函数仅用于在测试中验证物品分类。
    #[inline]
    #[must_use]
    pub fn is_axe(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_AXES)
    }

    #[inline]
    #[must_use]
    pub fn is_pickaxe(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_PICKAXES)
    }

    #[inline]
    #[must_use]
    pub fn is_shovel(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_SHOVELS)
    }

    #[inline]
    #[must_use]
    pub fn is_hoe(&self) -> bool {
        self.item.has_tag(&tag::Item::MINECRAFT_HOES)
    }
}
