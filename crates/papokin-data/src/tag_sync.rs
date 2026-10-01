//! 标签数据包（UpdateTags）按版本的下发策略。
//!
//! 数据集（26.3）的注册表 id 只有在「目标客户端注册表 id 空间与标签
//! id 空间一致」时才能直接下发。现状盘点：
//! - block / item / entity_type 有 id 重映射表（`*_id_remap`）；
//! - 拥有按版本数据包文件夹的注册表（worldgen/biome、damage_type、
//!   banner_pattern、instrument、painting_variant、enchantment、
//!   dialog、timeline 等）的标签 id 在生成期取各版本文件夹自身的
//!   字母序——与登录同步表（`REGISTRY_V_*`）同源同排序，客户端以
//!   同步表为注册表，因此天然对齐。2026-10-01 勘误：此前生成器只
//!   从 26_3 文件夹取一次 id 表，群系（1.18+，65 项中 57 项错位）、
//!   damage_type（1.19.4+）、enchantment（1.20.5–1.21.9）、
//!   painting_variant（1.20.5–1.21.6）下发给旧客户端的标签 id 全部
//!   落在 26.3 同步空间，与其登录注册表错位（与流体换序同族病）；
//! - **fluid**：流体注册序自 1.13 起稳定为 `empty/flowing_water/water/
//!   flowing_lava/lava`，数据集（26.x）与 1.13–1.21.11 原版**完全同序**，
//!   标签 id 原样下发即可，无需换序。2026-10-01 勘误：上一轮「water
//!   与 flowing_lava 两版互换」的结论有误（原版 `Fluids.java` 注册序与
//!   数据集一致），据此加入的 2↔3 换序反而把 water 装进了
//!   `minecraft:lava` 标签——客户端的眼睛入液判定走 FluidTags，导致
//!   旧客户端浸水时渲染岩浆红屏且不掉血（见 note/15 热修章节勘误）；
//! - **game_event**：冻结注册表且无逐版本真值。26.2 新增的 `bounce`
//!   插在 26.3 序第 9 位（非尾部追加），26.1 客户端其余事件 id 整体
//!   偏移一位；26.2 与 26.3 事件集合零漂移视为同序。26.2 之前整体
//!   省略（1.17+ 为按名寻址的变长格式，可安全省略）——客户端保留
//!   自身内建的原版标签，天然正确；
//! - **potion**：冻结注册表，`assets/potion.json` 即原版序（经典序 +
//!   1.20.5 尾部追加的 4 种），26.1 起才有标签文件夹，更早版本无
//!   标签可发（`network_tag_keys` 自然排除），id 无需换算；
//! - **point_of_interest_type / cat_variant / 1.20.5 前的
//!   banner_pattern、painting_variant 与 1.21.2 前的 instrument**：
//!   这些组合下注册表在客户端为冻结内建（本服不发送同步表），
//!   内建注册序无逐版本真值，下发必然引入不可验证的 id 猜测——
//!   一律整体省略，客户端保留内建标签（内建标签与内建注册表同源，
//!   天然一致）。cat_variant 在 1.21.5 起原版自身也不再同步标签。

use papokin_util::version::JavaMinecraftVersion;

use crate::tag::RegistryKey;

/// 该注册表的标签能否安全下发给该版本的客户端。
///
/// 返回 `false` 的注册表应从 UpdateTags 数据包中整体省略
/// （客户端保留内建标签），而不是发送空表或数据集 id。
/// 仅适用于 1.17+ 的变长按名格式；1.13–1.16.5 的定长位置
/// 格式（Block/Item/Fluid[/EntityType]）不能省略任何段。
#[must_use]
pub fn tag_registry_sendable_for_version(key: RegistryKey, version: JavaMinecraftVersion) -> bool {
    match key {
        // 冻结注册表：26.2 的 bounce 插入 26.3 序第 9 位，26.1 及更早
        // 的客户端 id 整体偏移且无逐版本映射数据，整体省略。
        RegistryKey::GameEvent => version >= JavaMinecraftVersion::V_26_2,
        // 冻结注册表且无逐版本真值（硬编码表无从对证），原版客户端
        // 也没有兴趣点标签的消费方，整体省略以保留内建标签。
        RegistryKey::PointOfInterestType => false,
        // 1.19–1.21.4 为冻结内建注册表（无真值）；1.21.5 起原版自身
        // 不再同步 cat_variant 标签（各版本数据包 tags 文件夹消失），
        // 省略即原版行为。
        RegistryKey::CatVariant => false,
        // 1.19 起有标签文件夹，但彼时注册表仍为冻结内建（无逐版本
        // 真值）；注册表文件夹分别自 1.20.5（banner/painting）与
        // 1.21.2（instrument）才出现——仅在本服同步该注册表、标签
        // id 落在同步表空间的版本下发。
        RegistryKey::BannerPattern | RegistryKey::PaintingVariant => {
            version >= JavaMinecraftVersion::V_1_20_5
        }
        RegistryKey::Instrument => version >= JavaMinecraftVersion::V_1_21_2,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use papokin_util::version::JavaMinecraftVersion as V;

    use super::tag_registry_sendable_for_version as sendable;
    use crate::tag::RegistryKey;

    /// 冻结注册表无真值的组合整体省略；同步类注册表自同步起始版本起可发。
    #[test]
    fn sendable_gates_match_registry_sync_eras() {
        // game_event：26.1 客户端因 bounce 插入位错位，26.2 起与数据集同集合。
        assert!(!sendable(RegistryKey::GameEvent, V::V_1_21_11));
        assert!(!sendable(RegistryKey::GameEvent, V::V_1_17));
        assert!(!sendable(RegistryKey::GameEvent, V::V_26_1));
        assert!(sendable(RegistryKey::GameEvent, V::V_26_2));
        assert!(sendable(RegistryKey::GameEvent, V::V_26_3));
        // 兴趣点与猫变种：任何版本都无逐版本真值，全省略。
        assert!(!sendable(RegistryKey::PointOfInterestType, V::V_1_21_11));
        assert!(!sendable(RegistryKey::PointOfInterestType, V::V_26_3));
        assert!(!sendable(RegistryKey::CatVariant, V::V_26_1));
        // 冻结时代省略、同步时代下发的三类。
        assert!(!sendable(RegistryKey::BannerPattern, V::V_1_19_4));
        assert!(!sendable(RegistryKey::PaintingVariant, V::V_1_20_2));
        assert!(sendable(RegistryKey::BannerPattern, V::V_1_20_5));
        assert!(sendable(RegistryKey::PaintingVariant, V::V_1_20_5));
        assert!(!sendable(RegistryKey::Instrument, V::V_1_21));
        assert!(sendable(RegistryKey::Instrument, V::V_1_21_2));
        // 有重映射表或与数据集同序的注册表始终可发。
        assert!(sendable(RegistryKey::Block, V::V_1_13));
        assert!(sendable(RegistryKey::Fluid, V::V_1_13));
        assert!(sendable(RegistryKey::DamageType, V::V_1_19_4));
        assert!(sendable(RegistryKey::WorldgenBiome, V::V_1_18));
    }
}
