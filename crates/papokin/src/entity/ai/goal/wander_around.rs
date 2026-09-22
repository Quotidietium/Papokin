use super::{Controls, Goal, to_goal_ticks};
use crate::entity::ai::util::default_random_pos;
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob};
use papokin_util::math::vector3::Vector3;
use rand::RngExt;

pub const DEFAULT_INTERVAL: i32 = 120;

pub struct WanderAroundGoal {
    goal_control: Controls,
    speed: f64,
    target: Option<Vector3<f64>>,
    interval: i32,
    force_trigger: bool,
}

impl WanderAroundGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self::with_interval(speed, DEFAULT_INTERVAL)
    }

    #[must_use]
    pub const fn with_interval(speed: f64, interval: i32) -> Self {
        Self {
            goal_control: Controls::MOVE,
            speed,
            target: None,
            interval,
            force_trigger: false,
        }
    }

    /// 在下一次检查时运行一次，忽略间隔。
    pub const fn trigger(&mut self) {
        self.force_trigger = true;
    }

    pub const fn set_interval(&mut self, interval: i32) {
        self.interval = interval;
    }

    fn get_position(mob: &dyn Mob) -> Option<Vector3<f64>> {
        default_random_pos::get_pos(mob, 10, 7)
    }
}

impl Goal for WanderAroundGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        // 载着乘客的每只生物都由该乘客控制
        if mob.get_entity().has_passengers() {
            return false;
        }

        if !self.force_trigger {
            // TODO: 还要在长时间无动作时退出，这需要消失（despawn）记账。
            if mob
                .get_random()
                .random_range(0..to_goal_ticks(self.interval))
                != 0
            {
                return false;
            }
        }

        self.target = Self::get_position(mob);
        if self.target.is_none() {
            return false;
        }
        self.force_trigger = false;
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let idle = mob.is_navigator_idle();
        !idle && !mob.get_entity().has_passengers()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            let pos = mob.get_mob_entity().living_entity.entity.pos.load();
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(pos, target, self.speed));
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
