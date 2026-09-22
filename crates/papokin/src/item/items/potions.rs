use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::{
    lingering_potion::LingeringPotionEntity, splash_potion::SplashPotionEntity,
};
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::Sound;

pub struct PotionItem;
pub struct SplashPotionItem;
pub struct LingeringPotionItem;

impl ItemMetadata for PotionItem {
    fn ids() -> Box<[u16]> {
        [Item::POTION.id].into()
    }
}

impl ItemMetadata for SplashPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::SPLASH_POTION.id].into()
    }
}

impl ItemMetadata for LingeringPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::LINGERING_POTION.id].into()
    }
}

const POWER: f32 = 0.5;

impl ItemBehaviour for PotionItem {
    fn normal_use(&self, _item: &Item, _player: &Player) {
        // 饮用由服务器中的消耗品流程处理（激活的手 + 消耗刻）。
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for SplashPotionItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntityWitchThrow,
            papokin_data::sound::SoundCategory::Neutral,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::SPLASH_POTION);
        let splash = SplashPotionEntity::new_shot(entity, player.get_entity());

        // 将手持物品堆数据复制到弹射物中
        let main_s = player.inventory.held_item();
        let mut used_main = true;
        let mut stack = (!main_s.is_empty()
            && main_s.item.id == papokin_data::item::Item::SPLASH_POTION.id)
            .then_some(main_s);
        if stack.is_none() {
            let off_s = player.inventory.off_hand_item();
            if !off_s.is_empty() && off_s.item.id == papokin_data::item::Item::SPLASH_POTION.id {
                stack = Some(off_s);
                used_main = false;
            }
        }
        let stack = stack.unwrap_or_else(|| ItemStack::EMPTY.clone());
        splash.set_item_stack(stack);

        let (yaw, pitch) = player.rotation();
        splash.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);

        world.spawn_entity(Arc::new(splash));

        // 递减已用的物品堆（清空）
        if used_main {
            let mut s = player.inventory.held_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_held_item(s);
        } else {
            let mut s = player.inventory.off_hand_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player
                .inventory
                .set_stack_in_hand(papokin_util::Hand::Left, s);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for LingeringPotionItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntityWitchThrow,
            papokin_data::sound::SoundCategory::Neutral,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::LINGERING_POTION);
        let ling = LingeringPotionEntity::new_shot(entity, player.get_entity());

        // 将手持物品堆数据复制到弹射物中
        let main_s = player.inventory.held_item();
        let mut used_main = true;
        let mut stack = (!main_s.is_empty()
            && main_s.item.id == papokin_data::item::Item::LINGERING_POTION.id)
            .then_some(main_s);
        if stack.is_none() {
            let off_s = player.inventory.off_hand_item();
            if !off_s.is_empty() && off_s.item.id == papokin_data::item::Item::LINGERING_POTION.id {
                stack = Some(off_s);
                used_main = false;
            }
        }
        let stack = stack.unwrap_or_else(|| ItemStack::EMPTY.clone());
        ling.set_item_stack(stack);

        let (yaw, pitch) = player.rotation();
        ling.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);

        world.spawn_entity(Arc::new(ling));

        // 递减已用的物品堆（清空）
        if used_main {
            let mut s = player.inventory.held_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_held_item(s);
        } else {
            let mut s = player.inventory.off_hand_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player
                .inventory
                .set_stack_in_hand(papokin_util::Hand::Left, s);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
