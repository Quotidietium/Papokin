use std::sync::Arc;
use tokio::sync::Mutex;
use wasmtime::component::Resource;

use crate::plugin::{
    enchantment::{enchant_item::EnchantItemEvent, prepare_item_enchant::PrepareItemEnchantEvent},
    loader::wasm::wasm_host::{
        state::{ItemStackResource, PluginHostState},
        wit::v0_1::{
            events::{ToFromWasmEvent, consume_player},
            item_stack::{from_wit_enchantment, to_wit_enchantment},
            papokin::plugin::event::{
                EnchantItemEventData, EnchantmentOffer, EnchantmentValue as WitEnchantmentValue,
                Event, PrepareItemEnchantEventData,
            },
        },
    },
};

fn consume_item_stack(
    state: &mut PluginHostState,
    item: &Resource<
        crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::item_stack::ItemStack,
    >,
) -> papokin_data::item_stack::ItemStack {
    let mutex = state
        .resource_table
        .delete::<ItemStackResource>(Resource::new_own(item.rep()))
        .expect("无效的物品堆资源句柄")
        .provider;
    mutex.try_lock().expect("锁定物品堆").clone()
}

impl ToFromWasmEvent for PrepareItemEnchantEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let item = state
            .add_item_stack(Arc::new(Mutex::new(self.item.clone())))
            .expect("添加物品堆资源失败");

        let offers = (0..3)
            .map(|i| EnchantmentOffer {
                cost: self.level_requirements[i],
                enchantment_id: self.enchantment_id[i],
                enchantment_level: self.enchantment_level[i],
            })
            .collect();

        Event::PrepareItemEnchantEvent(PrepareItemEnchantEventData {
            player,
            item,
            offers,
            bookshelf_count: self.bookshelf_count,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PrepareItemEnchantEvent(data) => {
                let mut level_requirements = [0; 3];
                let mut enchantment_id = [-1; 3];
                let mut enchantment_level = [-1; 3];
                for (i, offer) in data.offers.iter().enumerate().take(3) {
                    level_requirements[i] = offer.cost;
                    enchantment_id[i] = offer.enchantment_id;
                    enchantment_level[i] = offer.enchantment_level;
                }
                Self {
                    player: consume_player(state, &data.player),
                    item: consume_item_stack(state, &data.item),
                    level_requirements,
                    enchantment_id,
                    enchantment_level,
                    bookshelf_count: data.bookshelf_count,
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for EnchantItemEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("添加玩家资源失败");
        let item = state
            .add_item_stack(Arc::new(Mutex::new(self.item.clone())))
            .expect("添加物品堆资源失败");

        let enchantments_to_add = self
            .enchantments_to_add
            .iter()
            .map(|(enc, level)| WitEnchantmentValue {
                enchantment: to_wit_enchantment(enc),
                level: *level as u32,
            })
            .collect();

        Event::EnchantItemEvent(EnchantItemEventData {
            player,
            item,
            option: self.option,
            cost: self.exp_level_cost,
            enchantments_to_add,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EnchantItemEvent(data) => {
                let enchantments_to_add = data
                    .enchantments_to_add
                    .into_iter()
                    .filter_map(|ev| {
                        // 事件回读路径容错：无效附魔条目跳过而非 panic
                        match from_wit_enchantment(ev.enchantment) {
                            Ok(enc) => Some((enc, ev.level as i32)),
                            Err(error) => {
                                tracing::error!("事件附魔 ID 无效，已跳过该条目：{error}");
                                None
                            }
                        }
                    })
                    .collect();
                Self {
                    player: consume_player(state, &data.player),
                    item: consume_item_stack(state, &data.item),
                    option: data.option,
                    exp_level_cost: data.cost,
                    enchantments_to_add,
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}
