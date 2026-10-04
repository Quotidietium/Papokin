//! 马系坐骑（马/驴/骡/羊驼/行商羊驼/骷髅马/僵尸马）共享的驯化机制。
//!
//! 对齐原版 `AbstractHorse`：空手上未驯服坐骑的背会发起驯化尝试——
//! 坐骑先载着玩家自由狂奔一段随机时间（`RunAroundLikeCrazyGoal`），
//! 随后结算：温顺度 +5，并以「随机数 < 新温顺度」判定成败；失败则
//! 尥蹶子（前蹄腾空）并把玩家摔下，成功则挂上主人、保留骑乘。
//! 未驯服时持非食物物品右键只会被发怒拒绝（原版 `makeMad`）。

use std::sync::atomic::{AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use papokin_data::particle::Particle;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::mob::Mob;

/// 原版每次驯化尝试提升的温顺度（`AbstractHorse` 的 `modifyTemper(5)`）。
const TEMPER_STEP: i32 = 5;
/// 上马到结算的延迟区间（刻）：先自由狂奔片刻再结算。
const ATTEMPT_DELAY_MIN: i32 = 20;
const ATTEMPT_DELAY_MAX: i32 = 60;
/// 尥蹶子（前蹄腾空）动画持续的刻数（对齐原版 `standFor`）。
const STANDING_TICKS: i32 = 20;

/// 温顺度夹在 `0..=max`（原版 `modifyTemper` 的取值域）。
fn clamp_temper(value: i32, max: i32) -> i32 {
    value.clamp(0, max)
}

/// 原版驯化判定：`随机数(0..上限) < 新温顺度` 时驯服成功。
const fn taming_roll_succeeds(roll: i32, temper: i32) -> bool {
    roll < temper
}

pub trait EquineTaming: Mob {
    /// 温顺度（0..=上限），每次驯化尝试 +5。
    fn equine_temper(&self) -> &AtomicI32;
    /// 驯服后的主人。
    fn equine_owner(&self) -> &AtomicCell<Option<Uuid>>;
    /// 驯化状态机：>0 为尝试倒计时，<0 为尥蹶子动画剩余刻数，0 为空闲。
    fn equine_taming_timer(&self) -> &AtomicI32;
    /// 温顺度上限：马/驴/骡/骷髅马/僵尸马为 100，羊驼系为 30。
    fn equine_max_temper(&self) -> i32 {
        100
    }
    /// 是否已驯服（对应各实体的 `FLAG_TAME`）。
    fn is_equine_tamed(&self) -> bool;
    fn equine_set_tame(&self, tame: bool);
    /// 前蹄腾空标志（同步到客户端的尥蹶子动画）。
    fn equine_set_standing(&self, standing: bool);
    /// 发怒音效。
    fn equine_angry_sound(&self) -> Sound;

    /// 温顺度增减（夹在 `0..=上限`），返回新值。
    fn modify_equine_temper(&self, delta: i32) -> i32 {
        let new_temper = clamp_temper(
            self.equine_temper().load(Ordering::Relaxed) + delta,
            self.equine_max_temper(),
        );
        self.equine_temper().store(new_temper, Ordering::Relaxed);
        new_temper
    }

    /// 未驯服个体被骑上时启动驯化尝试；已有尝试进行中时不重启。
    fn start_equine_taming_attempt(&self) {
        let timer = self.equine_taming_timer();
        if timer.load(Ordering::Relaxed) == 0 {
            let mut rng = rand::rng();
            timer.store(
                rng.random_range(ATTEMPT_DELAY_MIN..=ATTEMPT_DELAY_MAX),
                Ordering::Relaxed,
            );
        }
    }

    /// 原版 `makeMad`：未驯服时持非食物物品右键，尥蹶子发怒并拒绝上马。
    fn equine_make_mad(&self) {
        self.equine_set_standing(true);
        self.equine_taming_timer()
            .store(-STANDING_TICKS, Ordering::Relaxed);
        let entity = self.get_entity();
        let world = entity.world.load();
        world.play_sound(
            self.equine_angry_sound(),
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
        self.spawn_equine_taming_particles(false);
    }

    /// 每刻推进驯化尝试与尥蹶子动画；在各实体 `mob_tick` 中调用。
    fn equine_taming_tick(&self) {
        let timer = self.equine_taming_timer();
        let ticks = timer.load(Ordering::Relaxed);
        if ticks > 0 {
            // 骑手提前下马或已被驯服（如插件置位）时放弃本次尝试
            if !self.get_entity().has_passengers() || self.is_equine_tamed() {
                timer.store(0, Ordering::Relaxed);
                return;
            }
            let remaining = ticks - 1;
            timer.store(remaining, Ordering::Relaxed);
            if remaining == 0 {
                self.resolve_equine_taming_attempt();
            }
        } else if ticks < 0 {
            let remaining = ticks + 1;
            timer.store(remaining, Ordering::Relaxed);
            if remaining == 0 {
                self.equine_set_standing(false);
            }
        }
    }

    /// 结算一次驯化尝试：温顺度 +5 后按原版概率判定，败则摔下骑手。
    fn resolve_equine_taming_attempt(&self) {
        let entity = self.get_entity();
        let Some(rider) = entity.get_first_passenger() else {
            return;
        };
        let rider_id = rider.get_entity().entity_id;
        let world = entity.world.load();
        let new_temper = self.modify_equine_temper(TEMPER_STEP);
        let mut rng = rand::rng();
        if taming_roll_succeeds(rng.random_range(0..self.equine_max_temper()), new_temper) {
            // 驯服成功：保留骑乘，挂主人，播爱心与 EntityTameEvent 插件事件
            self.equine_set_tame(true);
            if let Some(player) = world.get_player_by_id(rider_id) {
                self.equine_owner().store(Some(player.gameprofile.id));
                self.tame(&player);
            }
            self.spawn_equine_taming_particles(true);
        } else {
            // 失败：尥蹶子 + 发怒音效 + 烟粒子 + 摔下骑手
            self.equine_set_standing(true);
            self.equine_taming_timer()
                .store(-STANDING_TICKS, Ordering::Relaxed);
            world.play_sound(
                self.equine_angry_sound(),
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            self.spawn_equine_taming_particles(false);
            entity.remove_passenger(rider_id);
        }
    }

    /// 驯化反馈粒子（成功爱心 / 失败烟），与 `tamable.rs` 同款广播方式。
    fn spawn_equine_taming_particles(&self, success: bool) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        let particle = if success {
            Particle::Heart
        } else {
            Particle::Smoke
        };
        world.spawn_particle(
            pos + Vector3::new(0.0, f64::from(entity.height()) * 0.5, 0.0),
            Vector3::new(0.5, 0.5, 0.5),
            0.02,
            7,
            particle,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temper_clamp_horse_bounds() {
        // 马系上限 100：每次尝试 +5，95 之后夹到 100，不为负
        assert_eq!(clamp_temper(TEMPER_STEP, 100), 5);
        assert_eq!(clamp_temper(95 + TEMPER_STEP, 100), 100);
        assert_eq!(clamp_temper(200, 100), 100);
        assert_eq!(clamp_temper(-5, 100), 0);
    }

    #[test]
    fn temper_clamp_llama_bounds() {
        // 原版羊驼上限 30
        assert_eq!(clamp_temper(25 + TEMPER_STEP, 30), 30);
        assert_eq!(clamp_temper(60, 30), 30);
    }

    #[test]
    fn taming_roll_boundaries() {
        // 原版判定「随机数 < 温顺度」：温顺度 0 永不成功，
        // 温顺度满上限时对 0..上限 的任意roll都成功
        assert!(!taming_roll_succeeds(0, 0));
        assert!(taming_roll_succeeds(0, 1));
        assert!(taming_roll_succeeds(4, 5));
        assert!(!taming_roll_succeeds(5, 5));
        assert!(taming_roll_succeeds(99, 100));
        assert!(taming_roll_succeeds(29, 30));
    }
}
