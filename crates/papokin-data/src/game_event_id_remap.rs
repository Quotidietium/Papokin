//! 游戏事件注册表 id 的跨版本重映射：把内置数据集（26.3）的
//! game_event id 翻译为客户端版本冻结注册表中的 id，供标签同步
//! （update-tags 数据包）按事件 id 引用注册表的发送路径使用。
//!
//! 26.3 序与 1.21.11 序的差异仅有两处插入、一处删除：
//! - 26.2 新增的 `bounce` 插在 26.3 序第 9 位（1.21.11 无此项）；
//! - 26.x 的 `jukebox_stop_play` 占 26.3 序第 32 位（1.21.11 无此项）；
//! - 1.21.11 的 `mob_interact`（第 31 位）在 26.x 已删除，26.3 序中
//!   不存在、无需映射来源。
//!
//! 因此 26.3→1.21.11 规则为：id≤8 恒等；id∈{9, 32} 映射为 0
//! （目标版本缺席，发送方按「映射为 0 即过滤」惯例丢弃）；其余
//! id−1。该规则已对照 Papo 1.21.11 实抓的 5 组游戏事件标签
//! （vibrations / ignore_vibrations_sneaking / warden_can_listen /
//! allay_can_listen / shrieker_can_listen）逐 id 验证全等。
//!
//! 26.2 起 26.x 家族与数据集零漂移，id 原样下发；26.1 及更早版本
//! （含 1.17–1.21.9）因 bounce 插入位整体错位且无逐版本真值，
//! 标签整体省略（见 [`crate::tag_sync`]），本函数不会被调用到，
//! 为安全起见同样返回入参。

use papokin_util::version::JavaMinecraftVersion;

/// 把数据集（26.3）的游戏事件 id 重映射为 `version` 客户端的 id。
/// 目标版本缺席的事件映射为 0（由发送方过滤）。
#[must_use]
pub const fn remap_game_event_id_for_version(id: u16, version: JavaMinecraftVersion) -> u16 {
    match version {
        JavaMinecraftVersion::V_1_21_11 => match id {
            0..=8 => id,
            // bounce(9) 与 jukebox_stop_play(32) 为 26.x 新增，1.21.11 缺席
            9 | 32 => 0,
            _ => id - 1,
        },
        // 26.2 与 26.3 事件集合零漂移；更早版本整体省略、不可达。
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::remap_game_event_id_for_version as remap;
    use papokin_util::version::JavaMinecraftVersion as V;

    /// 26.3→1.21.11 的映射结果必须与 Papo 1.21.11 实抓真值逐 id 全等：
    /// 静态表（26.3 空间）中的五组标签经映射后应还原出 1.21.11 空间 id。
    #[test]
    fn remap_matches_papo_1_21_11_ground_truth() {
        // shrieker_can_listen：sculk_sensor_tendrils_clicking 26.3=38 → 37
        assert_eq!(remap(38, V::V_1_21_11), 37);
        // allay_can_listen：note_block_play 26.3=34 → 33
        assert_eq!(remap(34, V::V_1_21_11), 33);
        // ignore_vibrations_sneaking：hit_ground 27→26、projectile_shoot
        // 37→36、step 42→41、swim 43→42、item_interact_start 30→29、
        // item_interact_finish 29→28
        assert_eq!(remap(27, V::V_1_21_11), 26);
        assert_eq!(remap(37, V::V_1_21_11), 36);
        assert_eq!(remap(42, V::V_1_21_11), 41);
        assert_eq!(remap(43, V::V_1_21_11), 42);
        assert_eq!(remap(30, V::V_1_21_11), 29);
        assert_eq!(remap(29, V::V_1_21_11), 28);
        // warden_can_listen 尾部：shriek 40→39
        assert_eq!(remap(40, V::V_1_21_11), 39);
        // 前 9 项（block_* 家族）两版同序，恒等
        for id in 0..=8u16 {
            assert_eq!(remap(id, V::V_1_21_11), id);
        }
        // 26.x 专属事件映射为 0（发送方过滤）
        assert_eq!(remap(9, V::V_1_21_11), 0);
        assert_eq!(remap(32, V::V_1_21_11), 0);
        // resonate_1..15：46..60 → 45..59
        assert_eq!(remap(46, V::V_1_21_11), 45);
        assert_eq!(remap(60, V::V_1_21_11), 59);
        // 26.x 原生客户端保持数据集 id
        assert_eq!(remap(38, V::V_26_2), 38);
        assert_eq!(remap(38, V::V_26_3), 38);
    }
}
