use std::sync::atomic::AtomicI32;

use papokin_util::math::vector3::Vector3;

use crate::entity::ai::goal::{Controls, Goal};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::ai::util::default_random_pos;
use crate::entity::mob::Mob;

pub struct PathfindToRaidGoal {
    recruitment_tick: AtomicI32,
    speed_modifier: f64,
}

impl Default for PathfindToRaidGoal {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl PathfindToRaidGoal {
    #[must_use]
    pub const fn new(speed_modifier: f64) -> Self {
        Self {
            recruitment_tick: AtomicI32::new(0),
            speed_modifier,
        }
    }
}

impl PathfindToRaidGoal {
    /// 仍在进行的袭击，且袭击者位于村庄之外。
    fn raid_is_calling(mob: &dyn Mob) -> bool {
        let Some(raider) = mob.as_raider() else {
            return false;
        };
        if !raider.has_active_raid() {
            return false;
        }
        let Some(raid_id) = raider.get_raider_data().raid_id.load() else {
            return false;
        };

        let pos = mob.get_entity().block_pos.load();
        let world = mob.get_entity().world.load();
        {
            let raids = world
                .raids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(raid) = raids.get(raid_id) else {
                return false;
            };
            if raid.is_over() {
                return false;
            }
        }

        // TODO: 这里应改为统计附近的 POI 区段。
        world
            .villager_poi
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_nearest_job_site(pos, 32)
            .is_none()
    }
}

impl Goal for PathfindToRaidGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        mob.get_mob_entity().get_target().is_none()
            && !mob.get_entity().has_passengers()
            && Self::raid_is_calling(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        // 即使出现目标，仍继续向袭击寻路。
        Self::raid_is_calling(mob)
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(raider) = mob.as_raider() else {
            return;
        };

        let Some(raid_id) = raider.get_raider_data().raid_id.load() else {
            return;
        };

        let entity = mob.get_entity();
        let world = entity.world.load();
        let current_age = entity.age.load(std::sync::atomic::Ordering::Relaxed);

        let raid_center = {
            let raids = world
                .raids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(raid) = raids.get(raid_id) else {
                return;
            };
            if raid.is_over() {
                return;
            }
            raid.center
        };

        // 定期招募附近的袭击者
        let next_recruit = self
            .recruitment_tick
            .load(std::sync::atomic::Ordering::Relaxed);
        if current_age >= next_recruit {
            self.recruitment_tick
                .store(current_age + 20, std::sync::atomic::Ordering::Relaxed);

            let bb = entity.bounding_box.load().expand(16.0, 16.0, 16.0);
            let nearby = world.get_entities_at_box(&bb);

            for cand in nearby {
                if cand.get_entity().entity_id != entity.entity_id
                    && let Some(cand_mob) = cand.get_mob()
                    && let Some(cand_raider) = cand_mob.as_raider()
                    && !cand_raider.has_active_raid()
                    && cand_raider.can_join_raid()
                {
                    cand_raider.get_raider_data().raid_id.store(Some(raid_id));
                }
            }
        }

        // 空闲时寻路前往袭击中心
        let is_idle = mob.is_navigator_idle();

        if is_idle {
            let raid_center = Vector3::new(
                f64::from(raid_center.0.x) + 0.5,
                f64::from(raid_center.0.y),
                f64::from(raid_center.0.z) + 0.5,
            );
            if let Some(dest) = default_random_pos::get_pos_towards(
                mob,
                15,
                4,
                raid_center,
                std::f64::consts::FRAC_PI_2,
            ) {
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(
                        mob.get_entity().pos.load(),
                        dest,
                        self.speed_modifier,
                    ));
            }
        }
    }
}
