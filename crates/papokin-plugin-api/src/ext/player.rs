use crate::wit::papokin::plugin::item_stack::ItemStack;
use crate::wit::papokin::plugin::player::{
    BanIpOptions, BanPlayerOptions, JavaKickOptions, Player, SocketTeardownPolicy,
};
use crate::wit::papokin::plugin::text::TextComponent;

/// 为玩家末影箱物品栏提供批量访问与辅助工具的扩展 trait。
pub trait PlayerEnderChestExt {
    ///返回玩家末影箱的全部 27 个槽位。
    fn get_all_ender_chest_items(&self) -> Vec<Option<ItemStack>>;

    /// 从迭代器设置玩家末影箱的全部 27 个槽位。
    fn set_all_ender_chest_items(&self, items: impl IntoIterator<Item = Option<ItemStack>>);
}

impl PlayerEnderChestExt for Player {
    fn get_all_ender_chest_items(&self) -> Vec<Option<ItemStack>> {
        (0..27)
            .map(|slot| self.get_ender_chest_item(slot))
            .collect()
    }

    fn set_all_ender_chest_items(&self, items: impl IntoIterator<Item = Option<ItemStack>>) {
        for (slot, item) in items.into_iter().take(27).enumerate() {
            self.set_ender_chest_item(slot as u8, item);
        }
    }
}

impl JavaKickOptions {
    /// 以给定原因和默认设置创建新的 `JavaKickOptions`。
    #[must_use]
    pub fn new(reason: TextComponent) -> Self {
        Self {
            reason,
            log_to_console: true,
            teardown_policy: SocketTeardownPolicy::Graceful,
        }
    }
}

impl Default for BanPlayerOptions {
    fn default() -> Self {
        Self {
            reason: None,
            source: None,
            expires_at_utc: None,
            duration_seconds: None,
            kick_if_online: true,
            log_to_console: true,
        }
    }
}

impl BanPlayerOptions {
    /// 创建新的永久封禁，可带可选原因，其余为默认设置。
    #[must_use]
    pub fn new(reason: Option<TextComponent>) -> Self {
        Self {
            reason,
            ..Default::default()
        }
    }

    /// 创建指定秒数时长的临时封禁。
    #[must_use]
    pub fn temporary(reason: Option<TextComponent>, duration_seconds: u64) -> Self {
        Self {
            reason,
            duration_seconds: Some(duration_seconds),
            ..Default::default()
        }
    }
}

impl Default for BanIpOptions {
    fn default() -> Self {
        Self {
            reason: None,
            source: None,
            expires_at_utc: None,
            duration_seconds: None,
            kick_matching_players: true,
            log_to_console: true,
        }
    }
}

impl BanIpOptions {
    /// 创建新的永久 IP 封禁，可带可选原因，其余为默认设置。
    #[must_use]
    pub fn new(reason: Option<TextComponent>) -> Self {
        Self {
            reason,
            ..Default::default()
        }
    }

    /// 创建指定秒数时长的临时 IP 封禁。
    #[must_use]
    pub fn temporary(reason: Option<TextComponent>, duration_seconds: u64) -> Self {
        Self {
            reason,
            duration_seconds: Some(duration_seconds),
            ..Default::default()
        }
    }
}

/// 为 `Player` 提供类型化冷却辅助的扩展 trait。
pub trait PlayerCooldownExt {
    /// 使用任意物品键或 `Item` 枚举设置客户端的物品冷却遮罩。
    fn set_cooldown(&self, item: impl crate::item::IntoItemKey, ticks: i32);

    /// 返回物品剩余的冷却刻数（如冷却激活中）。
    fn get_cooldown(&self, item: impl crate::item::IntoItemKey) -> Option<i32>;

    /// 检查物品当前是否在冷却中。
    fn has_cooldown(&self, item: impl crate::item::IntoItemKey) -> bool;
}

impl PlayerCooldownExt for Player {
    fn set_cooldown(&self, item: impl crate::item::IntoItemKey, ticks: i32) {
        self.set_item_cooldown(&item.into_item_key(), ticks);
    }

    fn get_cooldown(&self, item: impl crate::item::IntoItemKey) -> Option<i32> {
        self.get_item_cooldown(&item.into_item_key())
    }

    fn has_cooldown(&self, item: impl crate::item::IntoItemKey) -> bool {
        self.has_item_cooldown(&item.into_item_key())
    }
}

#[cfg(test)]
mod tests {
    use crate::{CustomStatistic, StatisticCategory};

    #[test]
    fn statistic_types() {
        assert_eq!(StatisticCategory::Mined as u8, 0);
        assert_eq!(StatisticCategory::Crafted as u8, 1);
        assert_eq!(StatisticCategory::Custom as u8, 8);

        assert_eq!(CustomStatistic::LeaveGame as u8, 0);
        assert_eq!(CustomStatistic::PlayTime as u8, 1);
        assert_eq!(CustomStatistic::Deaths as u8, 32);
        assert_eq!(CustomStatistic::PlayerKills as u8, 35);
    }
}
