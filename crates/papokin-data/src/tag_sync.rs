//! 标签数据包（UpdateTags）按版本的下发策略。
//!
//! 数据集（26.3）的注册表 id 只有在「目标客户端冻结注册表与数据集
//! 同序」或「存在逐版本重映射表」时才能直接下发。现状盘点：
//! - block / item / entity_type 有 id 重映射表（`*_id_remap`）；
//! - 拥有按版本数据包文件夹的注册表（worldgen/biome、damage_type、
//!   banner_pattern、instrument、painting_variant、enchantment、
//!   dialog、timeline 等）在生成期即取各版本自身的 id；
//! - potion 与 point_of_interest_type 无版本数据，但注册序自 1.13
//!   起稳定（药水为经典序；兴趣点按原版追加式注册，共享前缀一致）；
//! - **fluid**：数据集注册序 `empty/flowing_water/water/flowing_lava/
//!   lava` 与 1.13–1.21.11 原版序在 water 与 flowing_lava 上互换，
//!   由 [`remap_fluid_tag_id_for_version`] 在发送时换回（1.13–1.16.5
//!   的标签包为定长位置格式，fluid 段无法省略，只能换 id）；
//! - **game_event**：26.3 有 61 个事件而 1.21.11 仅 55 个，id 跨版本
//!   漂移且本地无逐版本映射数据。与其下发错乱 id，不如整体不下发
//!   （1.17+ 为按名寻址的变长格式，可安全省略）——客户端保留自身
//!   内建的原版标签，天然正确。

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
        // 仅 26.x 家族与数据集同序；1.21.11 及更早的客户端省略
        RegistryKey::GameEvent => version >= JavaMinecraftVersion::V_26_1,
        _ => true,
    }
}

/// 流体标签 id 换回目标客户端的注册序。
///
/// 数据集（26.x）：`empty=0, flowing_water=1, water=2, flowing_lava=3,
/// lava=4`；1.13–1.21.11 原版：`flowing_lava=2, water=3`，两者互换。
#[must_use]
#[allow(clippy::match_same_arms)]
pub fn remap_fluid_tag_id_for_version(id: u16, version: JavaMinecraftVersion) -> u16 {
    if version >= JavaMinecraftVersion::V_26_1 {
        return id;
    }
    match id {
        2 => 3,
        3 => 2,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use papokin_util::version::JavaMinecraftVersion as V;

    use super::{
        remap_fluid_tag_id_for_version as fluid, tag_registry_sendable_for_version as sendable,
    };
    use crate::tag::RegistryKey;

    /// game_event 标签仅对 26.x 家族客户端下发；其余注册表始终可发。
    #[test]
    fn game_event_tags_only_for_modern_clients() {
        assert!(!sendable(RegistryKey::GameEvent, V::V_1_21_11));
        assert!(!sendable(RegistryKey::GameEvent, V::V_1_17));
        assert!(sendable(RegistryKey::GameEvent, V::V_26_1));
        assert!(sendable(RegistryKey::GameEvent, V::V_26_3));
        assert!(sendable(RegistryKey::Block, V::V_1_13));
        assert!(sendable(RegistryKey::Fluid, V::V_1_13));
    }

    /// 数据集 fluid 序 water=2/flowing_lava=3 与 1.13–1.21.11 原版序
    /// 互换；26.x 家族恒等。
    #[test]
    fn fluid_tag_ids_swap_water_and_flowing_lava_for_old_clients() {
        assert_eq!(fluid(2, V::V_1_21_11), 3); // water -> 原版 3
        assert_eq!(fluid(3, V::V_1_21_11), 2); // flowing_lava -> 原版 2
        assert_eq!(fluid(0, V::V_1_13), 0);
        assert_eq!(fluid(1, V::V_1_16), 1);
        assert_eq!(fluid(4, V::V_1_21_11), 4);
        assert_eq!(fluid(2, V::V_26_1), 2);
        assert_eq!(fluid(3, V::V_26_3), 3);
    }
}
