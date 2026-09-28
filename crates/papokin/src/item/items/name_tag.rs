use std::sync::Arc;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::data_component_impl::CustomNameImpl;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;

pub struct NameTagItem;

impl ItemMetadata for NameTagItem {
    fn ids() -> Box<[u16]> {
        [Item::NAME_TAG.id].into()
    }
}

impl ItemBehaviour for NameTagItem {
    fn use_on_entity(&self, item: &mut ItemStack, player: &Player, entity: Arc<dyn EntityBase>) {
        let entity = entity.get_entity();
        if entity.entity_type.saveable
            && let Some(name) = item.get_data_component::<CustomNameImpl>()
        {
            // 名字经 CustomName 组件（网络侧 NBT 文本树）注入，深嵌套
            // 的 extra/translated 层可拼出巨量文本并随实体 NBT 永久
            // 持久化；原版命名牌名字同样很短，超限直接忽略。
            const MAX_NAME_CHARS: usize = 512;
            if name.name.clone().get_text().chars().count() > MAX_NAME_CHARS {
                return;
            }
            let world = entity.world.load();
            let Some(player_arc) = world.get_player_by_id(player.entity_id()) else {
                return;
            };
            let mut name_event =
                crate::plugin::api::events::player::player_name_entity::PlayerNameEntityEvent {
                    player: player_arc,
                    entity_id: entity.entity_id,
                    name: name.name.clone(),
                    cancelled: false,
                };
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut name_event);
            }
            if name_event.cancelled {
                return;
            }
            // TODO
            entity.set_custom_name(name_event.name);
            item.decrement_unless_creative(player.gamemode.load(), 1);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
