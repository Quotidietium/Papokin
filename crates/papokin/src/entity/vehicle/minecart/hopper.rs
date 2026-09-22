use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::block::entities::hopper::HopperBlockEntity;
use crate::entity::{Entity, player::Player};
use papokin_data::translation;
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;

use super::container::{self, MinecartInventory};

#[derive(Clone)]
pub(super) struct HopperMinecart {
    enabled: Arc<AtomicBool>,
    inventory: Arc<MinecartInventory>,
}

impl HopperMinecart {
    pub(super) fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            inventory: Arc::new(MinecartInventory::new(5)),
        }
    }

    pub(super) const fn inventory(&self) -> &Arc<MinecartInventory> {
        &self.inventory
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub(super) fn tick(&self, entity: &Entity) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let world = entity.world.load();
        let pos = entity.pos.load();
        let source_pos = BlockPos::floored(pos.x, pos.y + 1.5, pos.z);
        let inventory = &self.inventory;
        let cart_box = entity.bounding_box.load().expand(0.25, 0.0, 0.25);

        // 为每个活动的漏斗矿车每刻运行：只构建
        // 仅在插件实际监听时才触发事件。
        let target_valid = world
            .server
            .upgrade()
            .filter(|server| {
                server.plugin_manager.has_handlers::<
                    crate::plugin::api::events::entity::item_transporting_entity_validate_target::ItemTransportingEntityValidateTargetEvent,
                >()
            })
            .is_none_or(|server| {
                let mut event = crate::plugin::api::events::entity::item_transporting_entity_validate_target::ItemTransportingEntityValidateTargetEvent::new(
                    entity.entity_id,
                    source_pos,
                );
                server.plugin_manager.fire_blocking(&server, &mut event);
                !event.cancelled
            });

        if target_valid
            && let Some(block_entity) = world.get_block_entity(&source_pos)
            && let Some(source) = block_entity.get_inventory()
        {
            for slot in 0..source.size() {
                let stack = source.get_stack(slot);
                if stack.is_empty() || !source.can_transfer_to(inventory.as_ref(), slot, &stack) {
                    continue;
                }
                let backup = stack.clone();
                let one = source.remove_stack_specific(slot, 1);
                if HopperBlockEntity::add_one_item(source.as_ref(), inventory.as_ref(), &one) {
                    return;
                }
                source.set_stack(slot, backup);
            }
            return;
        }

        let suction_box = BoundingBox::new(
            Vector3::new(pos.x - 0.5, pos.y + 0.6875, pos.z - 0.5),
            Vector3::new(pos.x + 0.5, pos.y + 2.0, pos.z + 0.5),
        );
        if Self::pick_up_item_internal(&world, inventory, &suction_box) {
            return;
        }
        Self::pick_up_item_internal(&world, inventory, &cart_box);
    }

    fn pick_up_item_internal(
        world: &Arc<crate::world::World>,
        inventory: &Arc<MinecartInventory>,
        search_box: &BoundingBox,
    ) -> bool {
        for entity in world.get_entities_at_box(search_box) {
            let Some(item) = entity.get_item_entity() else {
                continue;
            };
            let (backup, one) = {
                let mut stack = item
                    .get_item_stack()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if stack.is_empty() {
                    continue;
                }
                (stack.clone(), stack.split(1))
            };
            if HopperBlockEntity::add_one_item(inventory.as_ref(), inventory.as_ref(), &one) {
                let is_empty = {
                    let stack = item
                        .get_item_stack()
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    stack.is_empty()
                };
                if is_empty {
                    item.get_entity().remove();
                }
                return true;
            }
            let mut stack = item
                .get_item_stack()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *stack = backup;
        }
        false
    }

    pub(super) fn interact(
        &self,
        custom_name: Option<TextComponent>,
        player: &Arc<Player>,
    ) -> bool {
        container::open(
            custom_name,
            player,
            &self.inventory,
            TextComponent::translate(translation::java::ENTITY_MINECRAFT_HOPPER_MINECART, []),
            true,
        )
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.inventory.write_nbt(nbt);
        nbt.put_bool("Enabled", self.enabled.load(Ordering::Relaxed));
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.inventory.read_nbt(nbt);
        self.enabled
            .store(nbt.get_bool("Enabled").unwrap_or(true), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::HopperMinecart;
    use papokin_inventory::Inventory;

    #[test]
    fn hopper_minecart_inventory_has_five_slots() {
        assert_eq!(HopperMinecart::new().inventory.size(), 5);
    }
}
