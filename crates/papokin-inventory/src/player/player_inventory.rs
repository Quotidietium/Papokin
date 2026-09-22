//! 玩家物品栏实现。
//!
//! 本模块实现玩家物品栏，其组成如下：
//! - 36 个主物品栏槽位（3 行 x 9 格 + 快捷栏）
//! - 装备槽位（盔甲 + 副手）
//!
//! 主物品栏的前 9 个槽位是快捷栏（可用数字键访问）。
//! 槽位 0-35 为主物品栏，槽位 40 为副手槽位。

use crate::entity_equipment::EntityEquipment;
use crate::screen_handler::InventoryPlayer;

use crate::inventory::{Clearable, Inventory};
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_protocol::java::client::play::CSetPlayerInventory;
use papokin_util::Hand;
use rustc_hash::FxHashMap;
use std::any::Any;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tracing::warn;

/// 玩家的物品栏。
///
/// 包含 36 个主物品栏槽位（快捷栏 + 主存储区），另有
/// 通过 [`EntityEquipment`] 访问的装备槽。
pub struct PlayerInventory {
    /// 36 个主物品栏槽位（槽位 0-35）。
    ///
    /// 前 9 个槽位（0-8）是快捷栏，其余 27 个（9-35）是主存储区。
    pub main_inventory: RwLock<[ItemStack; Self::MAIN_SIZE]>,
    /// 槽位索引到装备栏位类型的映射。
    ///
    /// 用于识别哪些槽位对应盔甲和副手装备。
    pub equipment_slots: Arc<FxHashMap<usize, EquipmentSlot>>,
    /// 当前选中的快捷栏槽位索引（0-8）。
    pub selected_slot: AtomicU8,
    /// 用于盔甲和副手物品的实体装备存储。
    ///
    /// 这与主物品栏是分开的，并渲染在玩家模型上。
    pub entity_equipment: Arc<Mutex<EntityEquipment>>,
}

impl PlayerInventory {
    /// 主物品栏的大小（36 个槽位：27 个存储槽 + 9 个快捷栏槽）。
    pub const MAIN_SIZE: usize = 36;
    /// 快捷栏的大小（9 个槽位）。
    const HOTBAR_SIZE: usize = 9;
    /// 副手槽位索引（40）。
    pub const OFF_HAND_SLOT: usize = 40;

    /// 创建一个新的玩家物品栏。
    ///
    /// # Arguments
    /// - `entity_equipment` - 用于盔甲/副手的实体装备存储
    /// - `equipment_slots` - 槽位索引到装备槽位的映射
    // TODO: 添加从 NBT 加载物品栏
    pub fn new(
        entity_equipment: Arc<Mutex<EntityEquipment>>,
        equipment_slots: Arc<FxHashMap<usize, EquipmentSlot>>,
    ) -> Self {
        Self {
            main_inventory: RwLock::new(std::array::from_fn(|_| ItemStack::EMPTY.clone())),
            equipment_slots,
            selected_slot: AtomicU8::new(0),
            entity_equipment,
        }
    }

    /// 快速非阻塞地统计某物品在主物品栏所有槽位中的数量。
    pub fn count_item(&self, item: &'static Item) -> u32 {
        let mut total = 0u32;
        if let Ok(inv) = self.main_inventory.try_read() {
            for stack in inv.iter() {
                if stack.get_item().id == item.id {
                    total += u32::from(stack.item_count);
                }
            }
        }
        total
    }

    /// 快速非阻塞地检查主物品栏是否包含给定物品。
    pub fn contains_item(&self, item: &'static Item) -> bool {
        if let Ok(inv) = self.main_inventory.try_read() {
            for stack in inv.iter() {
                if !stack.is_empty() && stack.get_item().id == item.id {
                    return true;
                }
            }
        }
        false
    }

    /// 获取当前选中的快捷栏槽位中的物品。
    ///
    /// 这是玩家当前在主手中持有的物品。
    pub fn held_item(&self) -> ItemStack {
        let inv = self
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inv[self.get_selected_slot() as usize].clone()
    }

    /// 设置当前选中快捷栏槽位中的物品。
    pub fn set_held_item(&self, stack: ItemStack) {
        let selected = self.get_selected_slot() as usize;
        let mut inv = self
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inv[selected] = stack;
    }

    /// 设置指定手中的物品。
    pub fn set_stack_in_hand(&self, hand: Hand, stack: ItemStack) {
        match hand {
            Hand::Right => self.set_held_item(stack),
            Hand::Left => {
                let Some(slot) = self.equipment_slots.get(&Self::OFF_HAND_SLOT) else {
                    return;
                };
                self.entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .put(slot, stack);
            }
        }
    }

    /// 获取指定手中的物品。
    ///
    /// # Arguments
    /// - `hand` - 从哪只手获取物品
    pub fn get_stack_in_hand(&self, hand: Hand) -> ItemStack {
        match hand {
            Hand::Left => self.off_hand_item(),
            Hand::Right => self.held_item(),
        }
    }

    /// 获取副手中的物品。
    ///
    /// Mojang 名称：`getOffHandStack`
    pub fn off_hand_item(&self) -> ItemStack {
        let Some(slot) = self.equipment_slots.get(&Self::OFF_HAND_SLOT) else {
            return ItemStack::EMPTY.clone();
        };
        self.entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(slot)
    }

    /// 交换主手与副手中的物品。
    ///
    /// # Returns
    /// 新的主手物品和新的副手物品。
    pub fn swap_item(&self) -> (ItemStack, ItemStack) {
        let Some(slot) = self.equipment_slots.get(&Self::OFF_HAND_SLOT) else {
            return (ItemStack::EMPTY.clone(), ItemStack::EMPTY.clone());
        };
        let mut equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selected = self.get_selected_slot() as usize;
        let mut main_inv = self
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let main_hand_item = main_inv[selected].clone();
        let new_main = equipment.put(slot, main_hand_item.clone());
        main_inv[selected] = new_main.clone();
        (new_main, main_hand_item)
    }

    /// 检查槽位索引是否为有效的快捷栏槽位。
    #[must_use]
    pub const fn is_valid_hotbar_index(slot: usize) -> bool {
        slot < Self::HOTBAR_SIZE
    }

    /// 将物品堆添加到任意可用槽位，优先与现有物品堆叠。
    fn add_stack(&self, stack: ItemStack) -> usize {
        let mut slot_index = self.get_occupied_slot_with_room_for_stack(&stack);

        if slot_index == -1 {
            slot_index = self.get_empty_slot();
        }

        if slot_index == -1 {
            stack.item_count as usize
        } else {
            self.add_stack_to_slot(slot_index as usize, stack)
        }
    }

    /// 将物品堆添加到指定槽位。
    ///
    /// 返回无法容纳的物品数量。
    fn add_stack_to_slot(&self, slot: usize, stack: ItemStack) -> usize {
        if slot >= Self::MAIN_SIZE {
            if let Some(slot_type) = self.equipment_slots.get(&slot) {
                let mut equipment = self
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let current = equipment.get(slot_type);
                if current.is_empty() {
                    equipment.put(slot_type, stack);
                    return 0;
                }
            }
            return stack.item_count as usize;
        }

        let mut inv = self
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut stack_count = stack.item_count;
        let self_stack = &mut inv[slot];

        if self_stack.is_empty() {
            *self_stack = stack.copy_with_count(0);
        }

        let count_left = self_stack.get_max_stack_size() - self_stack.item_count;
        let count_min = stack_count.min(count_left);

        if count_min != 0 {
            stack_count -= count_min;
            self_stack.increment(count_min);
        }
        stack_count as usize
    }

    /// 在物品栏中寻找一个空槽位。
    ///
    /// # Returns
    /// 槽位索引，若物品栏已满则为 -1。
    fn get_empty_slot(&self) -> i16 {
        let inv = self
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (i, stack) in inv.iter().enumerate() {
            if stack.is_empty() {
                return i as i16;
            }
        }
        -1
    }

    /// 检查一组物品能否叠加到已有物品堆上。
    fn can_stack_add_more(existing_stack: &ItemStack, stack: &ItemStack) -> bool {
        !existing_stack.is_empty()
            && existing_stack.are_items_and_components_equal(stack)
            && existing_stack.is_stackable()
            && existing_stack.item_count < existing_stack.get_max_stack_size()
    }

    /// 寻找物品类型相同且还有空间容纳更多物品的槽位。
    ///
    /// 依次检查所选槽位、副手，然后是其他槽位。
    fn get_occupied_slot_with_room_for_stack(&self, stack: &ItemStack) -> i16 {
        let selected = self.get_selected_slot() as usize;
        let inv = self
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Self::can_stack_add_more(&inv[selected], stack) {
            return selected as i16;
        }

        let off_hand = self.off_hand_item();
        if Self::can_stack_add_more(&off_hand, stack) {
            return Self::OFF_HAND_SLOT as i16;
        }

        for (i, item) in inv.iter().enumerate() {
            if Self::can_stack_add_more(item, stack) {
                return i as i16;
            }
        }

        -1
    }

    /// 将物品堆插入任意可用槽位。
    ///
    /// # Arguments
    /// - `stack` - 要插入的物品堆（原地修改）
    ///
    /// # Returns
    /// 如果插入了任何物品则为 `true`，否则为 `false`。
    pub fn insert_stack_anywhere(&self, stack: &mut ItemStack) -> bool {
        self.insert_stack(-1, stack)
    }

    /// 将物品堆插入指定槽位或任意槽位。
    ///
    /// # Arguments
    /// - `slot` - 槽位索引，-1 表示任意槽位
    /// - `stack` - 要插入的物品堆（原地修改）
    ///
    /// # Returns
    /// 如果插入了任何物品则为 `true`，否则为 `false`。
    pub fn insert_stack(&self, slot: i16, stack: &mut ItemStack) -> bool {
        if stack.is_empty() {
            return false;
        }

        let mut i;

        loop {
            i = stack.item_count;
            if slot == -1 {
                stack.set_count(self.add_stack(stack.clone()) as u8);
            } else {
                stack.set_count(self.add_stack_to_slot(slot as usize, stack.clone()) as u8);
            }

            if stack.is_empty() || stack.item_count >= i {
                break;
            }
        }

        stack.item_count < i
    }

    /// 查找第一个包含匹配物品堆的槽位。
    ///
    /// # Returns
    /// 槽位索引，若未找到则为 -1。
    pub fn get_slot_with_stack(&self, stack: &ItemStack) -> i16 {
        let inv = self
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (i, item) in inv.iter().enumerate() {
            if !item.is_empty() && item.are_items_and_components_equal(stack) {
                return i as i16;
            }
        }
        -1
    }

    /// 寻找一个空的快捷栏槽位用于交换物品。
    ///
    /// 首先查找空槽位，然后查找没有附魔的槽位。
    fn get_swappable_hotbar_slot(&self) -> usize {
        let inv = self
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selected_slot = self.get_selected_slot() as usize;
        for i in 0..Self::HOTBAR_SIZE {
            let check_index = (i + selected_slot) % 9;
            if inv[check_index].is_empty() {
                return check_index;
            }
        }

        selected_slot
    }

    /// 将物品堆与快捷栏中的物品交换。
    ///
    /// 寻找一个空的快捷栏槽位并把物品堆放入其中。
    pub fn swap_stack_with_hotbar(&self, stack: ItemStack) {
        let swappable = self.get_swappable_hotbar_slot();
        self.set_selected_slot(swappable as u8);
        let selected = self.get_selected_slot() as usize;
        let mut inv = self
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(empty_slot) = inv.iter().position(ItemStack::is_empty)
            && !inv[selected].is_empty()
        {
            inv[empty_slot] = inv[selected].clone();
        }

        inv[selected] = stack;
    }

    /// 交换两个槽位索引处的物品。
    pub fn swap_slot_with_hotbar(&self, slot: usize) {
        let swappable = self.get_swappable_hotbar_slot();
        self.set_selected_slot(swappable as u8);
        let selected = self.get_selected_slot() as usize;
        let mut inv = self
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inv.swap(selected, slot);
    }

    /// 获取指定槽位中的物品（同步）。
    pub fn get_slot(&self, slot: usize) -> ItemStack {
        if slot < Self::MAIN_SIZE {
            let inv = self
                .main_inventory
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inv[slot].clone()
        } else if let Some(slot_type) = self.equipment_slots.get(&slot) {
            self.entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(slot_type)
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    /// 设置指定槽位中的物品（同步执行）。
    pub fn set_slot(&self, slot: usize, stack: ItemStack) {
        if slot < Self::MAIN_SIZE {
            let mut inv = self
                .main_inventory
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inv[slot] = stack;
        } else if let Some(slot_type) = self.equipment_slots.get(&slot) {
            self.entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .put(slot_type, stack);
        }
    }

    /// 将物品堆给予玩家，若物品栏已满则将其掉落。
    pub fn offer_or_drop_stack(&self, stack: ItemStack, player: &dyn InventoryPlayer) {
        self.offer(stack, true, player);
    }

    /// 将物品堆给予玩家，可选择是否通知客户端。
    ///
    /// # Arguments
    /// - `stack` - 要给予的物品堆
    /// - `notify_client` - 是否发送物品栏更新数据包
    /// - `player` - 要给予该物品堆的玩家
    pub fn offer(&self, stack: ItemStack, notify_client: bool, player: &dyn InventoryPlayer) {
        let mut stack = stack;
        while !stack.is_empty() {
            let mut room_for_stack = self.get_occupied_slot_with_room_for_stack(&stack);
            if room_for_stack == -1 {
                room_for_stack = self.get_empty_slot();
            }

            if room_for_stack == -1 {
                player.drop_item(stack, false);
                break;
            }

            let items_fit =
                stack.get_max_stack_size() - self.get_stack(room_for_stack as usize).item_count;
            if self.insert_stack(room_for_stack, &mut stack.split(items_fit)) && notify_client {
                player.enqueue_slot_set_packet(&CSetPlayerInventory::new(
                    i32::from(room_for_stack).into(),
                    &self.get_stack(room_for_stack as usize).into(),
                ));
            }
        }
    }
}

impl Clearable for PlayerInventory {
    fn clear(&self) {
        let mut inv = self
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inv.fill_with(|| ItemStack::EMPTY.clone());
        self.entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

impl Inventory for PlayerInventory {
    fn size(&self) -> usize {
        Self::MAIN_SIZE + self.equipment_slots.len()
    }

    fn is_empty(&self) -> bool {
        let inv = self
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if inv.iter().any(|s| !s.is_empty()) {
            return false;
        }

        for slot in self.equipment_slots.values() {
            let eq_item = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(slot);
            if !eq_item.is_empty() {
                return false;
            }
        }

        true
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        if slot < Self::MAIN_SIZE {
            let inv = self
                .main_inventory
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inv[slot].clone()
        } else if let Some(slot) = self.equipment_slots.get(&slot) {
            self.entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(slot)
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot < Self::MAIN_SIZE {
            let mut inv = self
                .main_inventory
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::replace(&mut inv[slot], ItemStack::EMPTY.clone())
        } else if let Some(slot) = self.equipment_slots.get(&slot) {
            self.entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .put(slot, ItemStack::EMPTY.clone())
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot < Self::MAIN_SIZE {
            let mut inv = self
                .main_inventory
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !inv[slot].is_empty() && amount > 0 {
                inv[slot].split(amount)
            } else {
                ItemStack::EMPTY.clone()
            }
        } else if let Some(slot) = self.equipment_slots.get(&slot) {
            let mut equipment = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut stack = equipment.get(slot);

            if !stack.is_empty() && amount > 0 {
                let split = stack.split(amount);
                equipment.put(slot, stack);
                split
            } else {
                ItemStack::EMPTY.clone()
            }
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot < Self::MAIN_SIZE {
            let mut inv = self
                .main_inventory
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inv[slot] = stack;
        } else if let Some(slot) = self.equipment_slots.get(&slot) {
            self.entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .put(slot, stack);
        } else {
            warn!("无法获取槽位 {slot} 处的装备槽");
        }
    }

    fn mark_dirty(&self) {}

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PlayerInventory {
    /// 设置选中的快捷栏槽位。
    ///
    /// # Panics
    /// 如果槽位索引不是有效的快捷栏索引则 panic。
    pub fn set_selected_slot(&self, slot: u8) {
        if Self::is_valid_hotbar_index(slot as usize) {
            self.selected_slot.store(slot, Ordering::Relaxed);
        }
    }

    /// 获取当前选中的快捷栏槽位索引。
    pub fn get_selected_slot(&self) -> u8 {
        self.selected_slot.load(Ordering::Relaxed)
    }
}
