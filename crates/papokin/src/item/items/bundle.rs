use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::data_component_impl::BundleContentsImpl;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::Sound;
use papokin_data::tag;
use papokin_util::Hand;

pub struct BundleItem;

impl ItemMetadata for BundleItem {
    fn ids() -> Box<[u16]> {
        tag::Item::MINECRAFT_BUNDLES.1.into()
    }
}

impl ItemBehaviour for BundleItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        // 取出-写回在写锁内原子完成（try_extract 只改本地克隆，
        // 真正落库走 update_held）：右键期间并发并入该槽位的物品
        // 不会被陈旧快照覆盖
        fn extract(mut s: ItemStack) -> (ItemStack, Option<ItemStack>) {
            let is_bundle = !s.is_empty() && BundleItem::ids().contains(&s.item.id);
            if !is_bundle {
                return (s, None);
            }
            let extracted = s
                .get_data_component_mut::<BundleContentsImpl>()
                .and_then(BundleContentsImpl::try_extract);
            (s, extracted)
        }

        let used_slot_index = player.inventory.get_selected_slot() as usize;
        let main_extracted = player.inventory.update_held(Hand::Right, extract);
        // 主手不是收纳袋才检查副手
        let (slot_index, extracted) = main_extracted.map_or_else(
            || {
                player
                    .inventory
                    .update_held(Hand::Left, extract)
                    .map_or_else(
                        || (used_slot_index, ItemStack::EMPTY.clone()),
                        |stack| (40, stack), // OFF_HAND_SLOT
                    )
            },
            |stack| (used_slot_index, stack),
        );

        if !extracted.is_empty() {
            let position = player.position();
            player.world().play_sound(
                Sound::ItemBundleRemoveOne,
                papokin_data::sound::SoundCategory::Players,
                &position,
            );
            player.drop_item(extracted);
            let updated_bundle = if slot_index == 40 {
                player.inventory.off_hand_item()
            } else {
                player.inventory.held_item()
            };
            player.sync_hand_slot(slot_index, updated_bundle);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
