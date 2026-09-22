//! 末影龙之战（DragonBattle）的查看与控制。
//!
//! 本模块在服务器的
//! `DragonFight` 管理器镜像 Paper 的 `DragonBattle` API。从末地世界
//! 通过 [`get_dragon_fight`] 或 [`WorldDragonFightExt`] 便捷
//! trait 获取 [`DragonFight`] 句柄；其他任何维度该调用都返回 `None`。
//!
//! 该句柄让插件查询被追踪的龙（[`DragonFight::get_dragon_uuid`]、
//! [`DragonFight::is_dragon_alive`]、[`DragonFight::has_been_killed_previously`]），
//! 观察并驱动重生动画（[`DragonFight::get_respawn_stage`]、
//! [`DragonFight::set_respawn_stage`]、[`DragonFight::initiate_respawn`]、
//! [`DragonFight::abort_respawn`]），查询存活的水晶柱与
//! 出口传送门位置，并生成龙之战的结构（水晶、出口
//! 传送门、折跃门）。
//!
//! 此处有意不再暴露龙之战的 Boss 血条；请改用
//! `boss-bar` WIT 接口。所有状态变化都经过
//! 原版使用的服务器端 `DragonFight` 代码路径。
//!
//! # Examples
//!
//! ## Querying the fight of the End world
//! ```rust,ignore
//! use papokin_plugin_api::dragon::WorldDragonFightExt;
//!
//! fn check(world: &papokin_plugin_api::wit::papokin::plugin::world::World) {
//!     let Some(fight) = world.get_dragon_fight() else {
//!         return; // Not The End.
//!     };
//!     if !fight.is_dragon_alive() && !fight.get_respawn_stage().is_some() {
//!         fight.initiate_respawn();
//!     }
//! }
//! ```

pub use crate::wit::papokin::plugin::common::BlockPos;
pub use crate::wit::papokin::plugin::dragon::{DragonFight, DragonRespawnStage, get_dragon_fight};
pub use crate::wit::papokin::plugin::uuid::Uuid;

use crate::wit::papokin::plugin::world::World;

/// 在 [`World`] 上获取其龙之战的扩展 trait。
pub trait WorldDragonFightExt {
    /// 返回此世界的 [`DragonFight`]；若该世界并非
    /// 末地（只有末地才有末影龙战斗）。
    #[must_use]
    fn get_dragon_fight(&self) -> Option<DragonFight>;
}

impl WorldDragonFightExt for World {
    fn get_dragon_fight(&self) -> Option<DragonFight> {
        get_dragon_fight(self)
    }
}
