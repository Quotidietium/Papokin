use std::any::Any;
use std::sync::Arc;

use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;

use crate::entity_equipment::EntityEquipment;
use crate::inventory::{Clearable, Inventory};

/// 槽位内容变化回调：用于同步实体标志（如马的 `FLAG_SADDLE`）
/// 与客户端装备渲染（`CSetEquipment`）。
pub type SlotChangedCallback = Arc<dyn Fn(&ItemStack) + Send + Sync>;

/// 把 [`EntityEquipment`] 中的单个装备槽适配为单格 [`Inventory`]，
/// 供坐骑界面（鞍/马铠槽）等 UI 复用。写入直接落到装备映射，
/// 变化经回调上报实体侧。
pub struct EquipmentSlotInventory {
    equipment: Arc<std::sync::Mutex<EntityEquipment>>,
    slot: EquipmentSlot,
    on_changed: Option<SlotChangedCallback>,
}

impl EquipmentSlotInventory {
    pub fn new(
        equipment: Arc<std::sync::Mutex<EntityEquipment>>,
        slot: EquipmentSlot,
    ) -> Arc<Self> {
        Arc::new(Self {
            equipment,
            slot,
            on_changed: None,
        })
    }

    /// 创建带变化回调的适配器（鞍槽装卸后同步实体标志与渲染）。
    pub fn with_callback(
        equipment: Arc<std::sync::Mutex<EntityEquipment>>,
        slot: EquipmentSlot,
        on_changed: SlotChangedCallback,
    ) -> Arc<Self> {
        Arc::new(Self {
            equipment,
            slot,
            on_changed: Some(on_changed),
        })
    }

    fn notify(&self, stack: &ItemStack) {
        if let Some(on_changed) = &self.on_changed {
            on_changed(stack);
        }
    }
}

impl Clearable for EquipmentSlotInventory {
    fn clear(&self) {
        let _ = self.remove_stack(0);
    }
}

impl Inventory for EquipmentSlotInventory {
    fn size(&self) -> usize {
        1
    }

    fn is_empty(&self) -> bool {
        self.get_stack(0).is_empty()
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        self.equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&self.slot)
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        let previous = self
            .equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put(&self.slot, ItemStack::EMPTY.clone());
        self.notify(&ItemStack::EMPTY.clone());
        previous
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        let mut current = self.get_stack(0);
        if current.is_empty() {
            return ItemStack::EMPTY.clone();
        }
        let removed = current.split(amount);
        let mut equipment = self
            .equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment.put(&self.slot, current.clone());
        drop(equipment);
        self.notify(&current);
        removed
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot != 0 {
            return;
        }
        self.equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put(&self.slot, stack);
        let current = self.get_stack(0);
        self.notify(&current);
    }

    fn count(&self, item: &Item) -> u8 {
        let stack = self.get_stack(0);
        if stack.item == item {
            stack.item_count
        } else {
            0
        }
    }

    /// 装备槽（鞍/马铠）只能容纳单件物品。
    fn get_max_count_per_stack(&self) -> u8 {
        1
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn equipment() -> Arc<std::sync::Mutex<EntityEquipment>> {
        Arc::new(Mutex::new(EntityEquipment::new()))
    }

    #[test]
    fn set_and_get_roundtrip_through_equipment_map() {
        let equipment = equipment();
        let inventory = EquipmentSlotInventory::new(equipment.clone(), EquipmentSlot::SADDLE);
        assert!(inventory.get_stack(0).is_empty());
        inventory.set_stack(0, ItemStack::new(1, &Item::SADDLE));
        assert!(
            equipment
                .lock()
                .unwrap()
                .get(&EquipmentSlot::SADDLE)
                .are_equal(&ItemStack::new(1, &Item::SADDLE))
        );
        assert!(
            inventory
                .get_stack(0)
                .are_equal(&ItemStack::new(1, &Item::SADDLE))
        );
    }

    #[test]
    fn change_callback_fires_with_new_contents() {
        let equipment = equipment();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorder = seen.clone();
        let inventory = EquipmentSlotInventory::with_callback(
            equipment,
            EquipmentSlot::BODY,
            Arc::new(move |stack| {
                recorder
                    .lock()
                    .unwrap()
                    .push(stack.get_item().registry_key.to_string());
            }),
        );
        inventory.set_stack(0, ItemStack::new(1, &Item::IRON_CHESTPLATE));
        let removed = inventory.remove_stack(0);
        assert!(!removed.is_empty());
        let seen = seen.lock().unwrap();
        assert_eq!(
            *seen,
            vec!["iron_chestplate".to_string(), "air".to_string()]
        );
    }

    #[test]
    fn out_of_range_slot_is_ignored() {
        let equipment = equipment();
        let inventory = EquipmentSlotInventory::new(equipment.clone(), EquipmentSlot::SADDLE);
        inventory.set_stack(7, ItemStack::new(1, &Item::SADDLE));
        assert!(
            equipment
                .lock()
                .unwrap()
                .get(&EquipmentSlot::SADDLE)
                .is_empty()
        );
        assert!(inventory.get_stack(3).is_empty());
        assert!(inventory.remove_stack(5).is_empty());
    }

    #[test]
    fn remove_stack_specific_splits_without_underflow() {
        let equipment = equipment();
        let inventory = EquipmentSlotInventory::new(equipment, EquipmentSlot::BODY);
        // 超堆叠防御：请求数量超过现存数量时只取走现存数量
        inventory.set_stack(0, ItemStack::new(1, &Item::IRON_CHESTPLATE));
        let removed = inventory.remove_stack_specific(0, 5);
        assert_eq!(removed.item_count, 1);
        assert!(inventory.get_stack(0).is_empty());
    }
}
