use std::any::Any;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;

pub struct CarrotOnAStickItem;
pub struct WarpedFungusOnAStickItem;

impl ItemMetadata for CarrotOnAStickItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CARROT_ON_A_STICK.id])
    }
}

impl ItemBehaviour for CarrotOnAStickItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let vehicle_opt = player
            .get_entity()
            .vehicle
            .try_lock()
            .ok()
            .and_then(|guard| guard.clone());
        if let Some(vehicle) = vehicle_opt
            && vehicle.get_entity().entity_type.id == EntityType::PIG.id
            && let Some(steerable) = vehicle.get_item_steerable()
            && steerable.boost()
        {
            // 原版：胡萝卜钓竿耐久耗尽直接损坏消失，不会返还钓鱼竿
            // （此前凭空给新钓竿属于物品捏造）
            player.damage_held_item(7);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ItemMetadata for WarpedFungusOnAStickItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::WARPED_FUNGUS_ON_A_STICK.id])
    }
}

impl ItemBehaviour for WarpedFungusOnAStickItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let vehicle_opt = player
            .get_entity()
            .vehicle
            .try_lock()
            .ok()
            .and_then(|guard| guard.clone());
        if let Some(vehicle) = vehicle_opt
            && vehicle.get_entity().entity_type.id == EntityType::STRIDER.id
            && let Some(steerable) = vehicle.get_item_steerable()
            && steerable.boost()
        {
            // 原版：诡异菌钓竿同理，损坏即消失
            player.damage_held_item(1);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
