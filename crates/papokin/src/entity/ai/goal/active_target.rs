use super::{Controls, Goal, to_goal_ticks};

use crate::entity::ai::goal::track_target::TrackTargetGoal;
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::living::LivingEntity;
use crate::entity::mob::Mob;
use crate::entity::{EntityBase, mob::MobEntity, player::Player};
use crate::world::World;
use papokin_data::attributes::Attributes;
use papokin_data::entity::EntityType;
use rand::RngExt;
use std::sync::Arc;

const DEFAULT_RECIPROCAL_CHANCE: i32 = 10;

pub struct ActiveTargetGoal {
    track_target_goal: TrackTargetGoal,
    target: Option<Arc<dyn EntityBase>>,
    reciprocal_chance: i32,
    target_type: Option<&'static EntityType>,
    target_predicate: TargetPredicate,
}

impl ActiveTargetGoal {
    pub fn new<F>(
        mob: &MobEntity,
        target_type: &'static EntityType,
        reciprocal_chance: i32,
        check_visibility: bool,
        check_can_navigate: bool,
        predicate: Option<F>,
    ) -> Self
    where
        F: Fn(&LivingEntity, &World) -> bool + Send + Sync + 'static,
    {
        let track_target_goal = TrackTargetGoal::new(check_visibility, check_can_navigate);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);

        if let Some(predicate) = predicate {
            target_predicate.set_predicate(predicate);
        }

        Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(reciprocal_chance),
            target_type: Some(target_type),
            target_predicate,
        }
    }

    #[must_use]
    pub fn with_default(
        mob: &MobEntity,
        target_type: &'static EntityType,
        check_visibility: bool,
    ) -> Box<Self> {
        let track_target_goal = TrackTargetGoal::with_default(check_visibility);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);

        Box::new(Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(DEFAULT_RECIPROCAL_CHANCE),
            target_type: Some(target_type),
            target_predicate,
        })
    }

    /// 瞄准通过 `predicate` 的任意类型中最近实体，类似原版的
    /// 不区分职业的 `NearestAttackableTargetGoal`（例如铁傀儡瞄准
    /// 除苦力怕外的所有 `Enemy`）。所有候选都会被检测，因此无效的
    /// 距该生物最近的实体不会阻挡更远的有效实体。
    pub fn predicated(
        mob: &MobEntity,
        reciprocal_chance: i32,
        check_visibility: bool,
        predicate: impl Fn(&LivingEntity, &World) -> bool + Send + Sync + 'static,
    ) -> Box<Self> {
        let track_target_goal = TrackTargetGoal::new(check_visibility, false);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        target_predicate.set_predicate(predicate);

        Box::new(Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(reciprocal_chance),
            target_type: None,
            target_predicate,
        })
    }

    pub fn set_target(&mut self, target: Option<Arc<dyn EntityBase>>) {
        self.target = target;
    }

    fn find_closest_target(&mut self, mob: &dyn Mob) {
        let mob_entity = mob.get_mob_entity();
        let follow_range = mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);

        // 原版在每次搜索时用当前跟随距离更新目标条件
        self.target_predicate.base_max_distance = follow_range;

        let world = mob_entity.living_entity.entity.world.load();

        // 原版使用 getEyeY() 搜索，因此我们按眼睛高度偏移位置
        let mut search_pos = mob_entity.living_entity.entity.pos.load();
        search_pos.y += mob_entity
            .living_entity
            .entity
            .entity_dimension
            .load()
            .eye_height as f64;

        // 选择通过条件的最近候选者，而非整体最近的。
        let predicate = &self.target_predicate;
        let found = if self.target_type == Some(&EntityType::PLAYER) {
            world
                .get_nearest_player(search_pos, follow_range, |player| {
                    predicate.test(&world, Some(mob), player.as_ref())
                })
                .map(|p: Arc<Player>| p as Arc<dyn EntityBase>)
        } else {
            let entity_types = self.target_type.map(|t| [t]);
            world.get_nearest_entity(
                search_pos,
                follow_range,
                entity_types.as_ref().map(<[&EntityType; 1]>::as_slice),
                |entity| predicate.test(&world, Some(mob), entity.as_ref()),
            )
        };

        self.target = found;
    }
}

impl Goal for ActiveTargetGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.reciprocal_chance > 0
            && mob.get_random().random_range(0..self.reciprocal_chance) != 0
        {
            return false;
        }
        self.find_closest_target(mob);
        self.target.is_some()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.track_target_goal.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.set_mob_target(self.target.clone());
        self.track_target_goal.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.track_target_goal.stop(mob);
    }

    fn controls(&self) -> Controls {
        self.track_target_goal.controls()
    }
}
