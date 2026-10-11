use crate::entity::player::Player;
use crate::item::ItemBehaviour;
use crate::item::ItemMetadata;
use papokin_data::data_component::DataComponent;
use papokin_data::data_component_impl::DataComponentImpl;
use papokin_data::data_component_impl::MapIdImpl;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_util::GameMode;
use std::any::Any;

pub struct MapItem;

impl ItemMetadata for MapItem {
    fn ids() -> Box<[u16]> {
        [Item::MAP.id].into()
    }
}

impl ItemBehaviour for MapItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let Some(server) = player.world().server.upgrade() else {
            return;
        };

        let inventory = player.inventory();
        // 手别探测借用化：只读判断哪只手持空地图，扣减/替换已在
        // update_held 写锁内自带匹配校验
        let mut found = false;
        let mut hand = papokin_util::Hand::Right;
        inventory.with_held_item(&mut |held| {
            found = !held.is_empty() && held.item.id == Item::MAP.id;
        });
        if !found {
            inventory.with_off_hand_item(&mut |off| {
                if !off.is_empty() && off.item.id == Item::MAP.id {
                    found = true;
                    hand = papokin_util::Hand::Left;
                }
            });
        }

        if found {
            let map_id = server.next_map_id();
            let _ = server.map_manager.create_map(
                map_id,
                player.world().dimension.clone(),
                player.position().x as i32,
                player.position().z as i32,
                0, // 默认缩放
            );

            let mut map_event =
                crate::plugin::api::events::server::map_initialize::MapInitializeEvent::new(map_id);
            server.plugin_manager.fire_blocking(&server, &mut map_event);

            let gamemode = player.gamemode.load();
            let make_filled_map = || {
                let mut map_stack = ItemStack::new(1, &Item::FILLED_MAP);
                map_stack.patch.push((
                    DataComponent::MapId,
                    Some(MapIdImpl { id: map_id }.to_dyn()),
                ));
                map_stack
            };
            // 读取-校验-扣减/替换-写回在写锁内原子完成（count==1 时
            // 原位换成成图地图）。返回：None = 槽位已不匹配（不作为）；
            // Some(true) = 已原位替换；Some(false) = 已扣减（需另给一张）
            let outcome = inventory.update_held(hand, |mut s| {
                let matched = !s.is_empty() && s.item.id == Item::MAP.id;
                if !matched {
                    return (s, None);
                }
                if s.item_count == 1 && gamemode != GameMode::Creative {
                    (make_filled_map(), Some(true))
                } else {
                    s.decrement_unless_creative(gamemode, 1);
                    (s, Some(false))
                }
            });

            if outcome == Some(false) {
                let mut stack_to_give = make_filled_map();
                let was_added = inventory.insert_stack_anywhere(&mut stack_to_give);
                if !was_added && !stack_to_give.is_empty() {
                    player
                        .world()
                        .drop_stack(&player.position().to_block_pos(), stack_to_give);
                }
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
