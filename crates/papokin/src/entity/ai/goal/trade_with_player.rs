use std::sync::atomic::Ordering;

use super::{Controls, Goal};
use crate::entity::mob::Mob;

const MAX_TRADE_DISTANCE_SQ: f64 = 16.0;

/// 玩家交易期间商人会站立不动；注视玩家是单独的目标。
#[derive(Default)]
pub struct TradeWithPlayerGoal;

impl TradeWithPlayerGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Goal for TradeWithPlayerGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = &mob.get_mob_entity().living_entity.entity;
        if !entity.is_alive()
            || entity.touching_water.load(Ordering::Relaxed)
            || !entity.on_ground.load(Ordering::Relaxed)
            || entity.velocity_dirty.load(Ordering::Relaxed)
        {
            return false;
        }

        let Some(player) = mob.get_trading_player() else {
            return false;
        };
        entity
            .pos
            .load()
            .squared_distance_to_vec(&player.living_entity.entity.pos.load())
            <= MAX_TRADE_DISTANCE_SQ
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        // 随后交易界面的有效性检查会自行关闭这笔交易。
        mob.clear_trading_player();
    }

    fn controls(&self) -> Controls {
        Controls::JUMP | Controls::MOVE
    }
}
