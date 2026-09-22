use std::sync::{Arc, Weak};

use papokin_data::damage::DamageType;
use papokin_data::effect::StatusEffect;
use papokin_data::entity::EntityType;
use papokin_data::item_stack::ItemStack;
use papokin_data::tag::{self, Taggable};

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    player::Player,
};

/// 鹦鹉吃下曲奇后所获中毒效果的持续刻数，与
/// 原版的 `Parrot.mobInteract`。
const COOKIE_POISON_DURATION: i32 = 900;

/// 表示鹦鹉，一种能模仿附近生物叫声的被动飞行生物。
///
/// Wiki: <https://minecraft.wiki/w/Parrot>
pub struct ParrotEntity {
    pub mob_entity: MobEntity,
}

impl ParrotEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let parrot = Self { mob_entity };
        let mob_arc = Arc::new(parrot);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    /// 给鹦鹉喂食曲奇：它会先中毒然后被杀死，与原版一致
    /// `Parrot.mobInteract`。
    fn eat_cookie(&self, player: &Arc<Player>, item_stack: &mut ItemStack) {
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);

        self.mob_entity
            .living_entity
            .add_effect(papokin_data::potion::Effect {
                effect_type: &StatusEffect::POISON,
                duration: COOKIE_POISON_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: true,
            });

        // 原版用 `player.isCreative() || !this.isInvulnerable()` 来守护此调用，
        // 但 `hurt` 自身会重新检查无敌状态，且 `player_attack` 不会绕过
        // 它，因此守卫只会跳过那些本来就不会产生任何效果的调用。
        self.damage_with_context(
            self,
            f32::MAX,
            DamageType::PLAYER_ATTACK,
            None,
            Some(player.as_ref()),
            Some(player.as_ref()),
        );
    }
}

impl Mob for ParrotEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        // 原版在驯服之后才最后检查有毒食物标签，这一点并
        // 实现。`parrot_food` 中没有任何内容同时出现在
        // `parrot_poisonous_food`，因此这两个分支不会混淆。
        if !item_stack
            .get_item()
            .has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD)
        {
            return self.mob_entity.mob_interact(player, item_stack);
        }

        self.eat_cookie(player, item_stack);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::COOKIE_POISON_DURATION;
    use papokin_data::item::Item;
    use papokin_data::tag::{self, Taggable};

    /// 此交互受原版 `parrot_poisonous_food` 标签限制，而不是
    /// 硬编码的 cookie id，因此请检查标签实际解析的方式是否符合
    /// 交互所假定的前提。
    #[test]
    fn cookie_is_poisonous_parrot_food() {
        assert!(Item::COOKIE.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    /// 在原版中种子可驯服鹦鹉，因此绝不能进入中毒分支。
    #[test]
    fn parrot_food_is_not_poisonous() {
        assert!(!Item::WHEAT_SEEDS.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
        assert!(!Item::COOKED_CHICKEN.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    #[test]
    fn poison_lasts_45_seconds() {
        assert_eq!(COOKIE_POISON_DURATION, 900);
    }
}
