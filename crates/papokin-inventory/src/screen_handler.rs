//! 屏幕处理器模块。
//!
//! 本模块定义容器 UI 的核心屏幕处理器系统。
//! 屏幕处理器管理容器界面的服务器端状态，
//! 处理槽位布局、点击处理、物品转移，以及
//! 并与客户端保持同步。
//!
//! # Core Components
//!
//! - [`ScreenHandler`] - 容器屏幕处理器的主 trait
//! - [`ScreenHandlerBehaviour`] - 所有屏幕处理器的共享状态
//! - [`InventoryPlayer`] - 玩家与容器交互的接口
//! - [`ScreenProperty`] - 容器 UI 属性（进度条等）
//!
//! # Screen Handler Lifecycle
//!
//! 1. 创建 - 创建屏幕处理器并配置槽位与同步 ID
//! 2. 打开 - 玩家打开容器，同步处理器随之挂接
//! 3. 交互 - 处理点击数据包，物品在槽位之间移动
//! 4. 关闭 - 容器关闭，光标物品被丢弃/交还玩家
//!
//! # Slot Indexing
//!
//! 槽位在每个屏幕处理器内从 0 开始编号。特殊值：
//! - `-1` - 光标槽位（手持物品）
//! - `-999` - 物品栏之外（丢弃到世界）

use crate::{
    container_click::MouseClick,
    player::player_inventory::PlayerInventory,
    slot::{NormalSlot, Slot},
    sync_handler::{SyncHandler, TrackedStack},
};
use crate::{
    inventory::{ComparableInventory, Inventory},
    window_property::PropertyDelegate,
};
use papokin_data::item_stack::ItemStack;
use papokin_data::{
    Enchantment,
    data_component_impl::{EquipmentSlot, EquipmentType, EquippableImpl},
    screen::WindowType,
    sound::Sound,
    statistic::StatisticCategory,
};
use papokin_protocol::{
    codec::item_stack_seralizer::OptionalItemStackHash,
    java::{
        client::play::{
            CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
            CSetPlayerInventory, CSetSelectedSlot,
        },
        server::play::SlotActionType,
    },
};
use papokin_util::text::TextComponent;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{any::Any, collections::HashMap, sync::Arc};
use tracing::warn;

/// 表示在物品栏外点击的槽位索引。
const SLOT_INDEX_OUTSIDE: i32 = -999;

/// 容器 UI 元素的一个被跟踪属性。
///
/// 属性用于同步 UI 状态，例如熔炉进度条，
/// 附魔等级，以及服务器与客户端之间的其他视觉指示。
pub struct ScreenProperty {
    old_value: i32,
    index: u8,
    value: Arc<dyn PropertyDelegate>,
}

impl ScreenProperty {
    /// 创建一个新的屏幕属性。
    ///
    /// # Arguments
    /// - `value` - 持有实际值的属性委托
    /// - `index` - 多值委托的属性索引
    pub fn new(value: Arc<dyn PropertyDelegate>, index: u8) -> Self {
        Self {
            old_value: value.get_property(i32::from(index)),
            index,
            value,
        }
    }

    /// 获取当前属性值。
    #[must_use]
    pub fn get(&self) -> i32 {
        self.value.get_property(i32::from(self.index))
    }

    /// 设置属性值。
    pub fn set(&mut self, value: i32) {
        self.value.set_property(i32::from(self.index), value);
    }

    /// 检查该值自上次检查以来是否发生变化。
    ///
    /// 将旧值更新为当前值。
    pub fn has_changed(&mut self) -> bool {
        let value = self.get();
        let has_changed = !value.eq(&self.old_value);
        self.old_value = value;
        has_changed
    }
}

/// 玩家与容器交互的接口。
///
/// 此 trait 抽象了玩家进行以下操作的能力：
/// - 将物品掉落到世界中
/// - 接收物品栏数据包
/// - 更换装备
/// - 获得经验
///
/// 实现者通常是能够打开容器的玩家实体。
pub trait InventoryPlayer: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    /// 将一件物品丢入世界。
    ///
    /// # Arguments
    /// - `item` - 要掉落的物品
    /// - `retain_ownership` - 若为 true，玩家保留所有权（用于拾取延迟）
    fn drop_item(&self, item: ItemStack, retain_ownership: bool);

    /// 获取玩家的物品栏。
    fn get_inventory(&self) -> Arc<PlayerInventory>;

    /// 检查玩家是否拥有无限材料（创造模式）。
    fn has_infinite_materials(&self) -> bool;

    /// 检查玩家是否处于创造模式。
    fn is_creative(&self) -> bool;

    /// 检查玩家是否处于旁观模式。
    fn is_spectator(&self) -> bool {
        false
    }

    /// 获取玩家的经验等级。
    fn experience_level(&self) -> i32;

    /// 增加或减少经验等级。
    fn add_experience_levels(&self, levels: i32);

    /// 获取玩家的附魔种子。
    fn enchantment_seed(&self) -> i32;

    /// 设置玩家的附魔种子。
    fn set_enchantment_seed(&self, seed: i32);

    /// 发送完整的容器内容数据包。
    fn enqueue_inventory_packet(
        &self,
        packet: &CSetContainerContent,
        window_type: Option<WindowType>,
    );

    /// 发送单个槽位更新数据包。
    fn enqueue_slot_packet(
        &self,
        packet: &CSetContainerSlot,
        window_type: Option<WindowType>,
        total_slots: usize,
    );

    /// 发送光标物品更新数据包。
    fn enqueue_cursor_packet(&self, packet: &CSetCursorItem);

    /// 发送属性更新数据包。
    fn enqueue_property_packet(&self, packet: &CSetContainerProperty);

    /// 发送玩家物品栏的槽位更新。
    fn enqueue_slot_set_packet(&self, packet: &CSetPlayerInventory);

    /// 发送选中槽位的更新。
    fn enqueue_set_held_item_packet(&self, packet: &CSetSelectedSlot);

    /// 发送装备变更数据包。
    fn enqueue_equipment_change(&self, slot: &EquipmentSlot, stack: &ItemStack);

    /// 给予玩家经验点（用于熔炉熔炼等）
    fn award_experience(&self, amount: i32);

    /// 为玩家增加一条统计数据。
    fn increment_stat(&self, category: StatisticCategory, stat_id: i32, amount: i32);

    /// 在打开的容器位置播放方块音效。
    fn play_block_sound(&self, sound: Sound, pitch: f32);

    /// 触发物品准备附魔事件。若被取消则返回 true。
    fn fire_prepare_item_enchant_event(
        &self,
        _item: &ItemStack,
        _level_requirements: &mut [i32; 3],
        _enchantment_id: &mut [i32; 3],
        _enchantment_level: &mut [i32; 3],
        _bookshelf_count: i32,
    ) -> bool {
        false
    }

    /// 触发物品附魔事件。若被取消则返回 true。
    fn fire_enchant_item_event(
        &self,
        _item: &ItemStack,
        _option: i32,
        _exp_level_cost: i32,
        _enchantments_to_add: &mut Vec<(&'static Enchantment, i32)>,
    ) -> bool {
        false
    }

    /// 关闭玩家当前打开的处理界面。
    fn close_screen_handler(&self) {}

    /// 执行铁砧方块损坏逻辑并播放铁砧音效事件。
    fn use_anvil(&self) {}

    /// 执行砂轮经验掉落并播放砂轮音效事件。
    fn use_grindstone(&self, _xp_amount: i32) {}
}

/// 将物品堆给予玩家，若物品栏已满则将其掉落。
///
/// 尝试先将物品堆插入玩家的物品栏，
/// 如果没有空间，则将其掉落在世界中。
pub fn offer_or_drop_stack(player: &dyn InventoryPlayer, stack: ItemStack) {
    // TODO: 原版的断开连接逻辑非常奇怪，稍后调查
    player.get_inventory().offer_or_drop_stack(stack, player);
}

/// 容器屏幕处理器的主要 trait。
///
/// 屏幕处理器管理箱子等容器 UI 的服务端状态，
/// 熔炉、工作台等。它们处理：
/// - 槽位布局与管理
/// - 点击处理
/// - 物品转移逻辑（Shift 点击）
/// - 客户端同步
///
/// # Implementation
///
/// 实现者必须提供：
/// - [`get_behaviour`](ScreenHandler::get_behaviour) 和 [`get_behaviour_mut`](ScreenHandler::get_behaviour_mut)
/// - [`quick_move`](ScreenHandler::quick_move) 用于 Shift 点击行为
/// - [`as_any`](ScreenHandler::as_any) 用于向下转型
// ScreenHandler.java
// TODO: 完整实现此功能
pub trait ScreenHandler: Send + Sync {
    // --- 同步方法 ---

    /// 获取此屏幕处理器的窗口类型。
    fn window_type(&self) -> Option<WindowType> {
        self.get_behaviour().window_type
    }

    /// 以 Any 引用形式返回此屏幕处理器。
    fn as_any(&self) -> &dyn Any;

    /// 以可变的 Any 引用形式返回此屏幕处理器。
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// 获取此屏幕处理器的同步 ID。
    fn sync_id(&self) -> u8 {
        self.get_behaviour().sync_id
    }

    /// 检查玩家能否使用此容器。
    fn can_use(&self, _player: &dyn InventoryPlayer) -> bool {
        true
    }

    /// 获取界面处理器行为的引用。
    fn get_behaviour(&self) -> &ScreenHandlerBehaviour;

    /// 获取界面处理器行为的可变引用。
    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour;

    /// 向此屏幕处理器添加一个槽位。
    ///
    /// 为该槽位分配 ID 并设置追踪。
    fn add_slot(&mut self, slot: Arc<dyn Slot>) -> Arc<dyn Slot> {
        let behaviour = self.get_behaviour_mut();
        slot.set_id(behaviour.slots.len());
        behaviour.slots.push(slot.clone());
        behaviour.tracked_stacks.push(ItemStack::EMPTY.clone());
        behaviour.previous_tracked_stacks.push(TrackedStack::EMPTY);

        slot
    }

    /// 从玩家物品栏添加快捷栏槽位（0-8）。
    fn add_player_hotbar_slots(&mut self, player_inventory: &Arc<dyn Inventory>) {
        for i in 0..9 {
            self.add_slot(Arc::new(NormalSlot::new(player_inventory.clone(), i)));
        }
    }

    /// 从玩家物品栏添加主物品栏槽位（9-35）。
    fn add_player_inventory_slots(&mut self, player_inventory: &Arc<dyn Inventory>) {
        for i in 0..3 {
            for j in 0..9 {
                self.add_slot(Arc::new(NormalSlot::new(
                    player_inventory.clone(),
                    j + (i + 1) * 9,
                )));
            }
        }
    }

    /// 添加玩家的所有物品栏槽位（主物品栏 + 快捷栏）。
    fn add_player_slots(&mut self, player_inventory: &Arc<dyn Inventory>) {
        self.add_player_inventory_slots(player_inventory);
        self.add_player_hotbar_slots(player_inventory);
    }

    /// 记录某槽位收到的哈希（用于同步跟踪）。
    fn set_received_hash(&mut self, slot: usize, hash: OptionalItemStackHash) {
        let behaviour = self.get_behaviour_mut();
        if slot < behaviour.previous_tracked_stacks.len() {
            behaviour.previous_tracked_stacks[slot].set_received_hash(hash);
        } else {
            warn!(
                "槽位索引不正确：{}，可用槽位数：{}",
                slot,
                behaviour.previous_tracked_stacks.len()
            );
        }
    }

    /// 记录某槽位收到的物品堆（用于同步跟踪）。
    fn set_received_stack(&mut self, slot: usize, stack: ItemStack) {
        let behaviour = self.get_behaviour_mut();
        behaviour.previous_tracked_stacks[slot].set_received_stack(stack);
    }

    /// 记录收到的光标哈希（用于同步跟踪）。
    fn set_received_cursor_hash(&mut self, hash: OptionalItemStackHash) {
        let behaviour = self.get_behaviour_mut();
        behaviour.previous_cursor_stack.set_received_hash(hash);
    }

    /// 添加一个要跟踪的属性。
    fn add_property(&mut self, property: ScreenProperty) {
        let behaviour = self.get_behaviour_mut();
        behaviour.properties.push(property);
        behaviour.tracked_property_values.push(0);
    }

    /// 添加多个要跟踪的属性。
    fn add_properties(&mut self, properties: Vec<ScreenProperty>) {
        for property in properties {
            self.add_property(property);
        }
    }

    /// 当玩家关闭容器时调用。
    ///
    /// 默认实现会丢弃光标物品。
    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        self.default_on_closed(player);
    }

    /// 默认关闭行为——丢弃光标上的物品。
    fn default_on_closed(&mut self, player: &dyn InventoryPlayer) {
        let behaviour = self.get_behaviour_mut();

        let mut cursor_stack_lock = behaviour
            .cursor_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !cursor_stack_lock.is_empty() {
            offer_or_drop_stack(player, cursor_stack_lock.clone());
            *cursor_stack_lock = ItemStack::EMPTY.clone();
        }
    }

    /// 将物品栏中的所有物品丢入世界。
    fn drop_inventory(&self, player: &dyn InventoryPlayer, inventory: Arc<dyn Inventory>) {
        for i in 0..inventory.size() {
            offer_or_drop_stack(player, inventory.remove_stack(i));
        }
    }

    /// 从另一个屏幕处理器复制受追踪的槽位状态。
    ///
    /// 在重新打开容器时用于恢复先前的状态。
    fn copy_shared_slots(&mut self, other: Arc<Mutex<dyn ScreenHandler>>) {
        let mut table: HashMap<ComparableInventory, HashMap<usize, usize>> = HashMap::new();
        let other_binding = other
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let other_behaviour = other_binding.get_behaviour();

        for i in 0..other_behaviour.slots.len() {
            let other_slot = other_behaviour.slots[i].clone();
            let mut hash_map = HashMap::new();
            hash_map.insert(other_slot.get_index(), i);
            table.insert(
                ComparableInventory(other_slot.get_inventory().clone()),
                hash_map,
            );
        }

        for i in 0..self.get_behaviour().slots.len() {
            let slot = self.get_behaviour().slots[i].clone();
            let inventory = slot.get_inventory();
            let index = slot.get_index();

            if let Some(hash_map) = table.get(&ComparableInventory(inventory.clone()))
                && let Some(other_index) = hash_map.get(&index)
            {
                self.get_behaviour_mut().tracked_stacks[i] =
                    other_behaviour.tracked_stacks[*other_index].clone();
                self.get_behaviour_mut().previous_tracked_stacks[i] =
                    other_behaviour.previous_tracked_stacks[*other_index].clone();
            }
        }
    }

    /// 将完整状态同步到客户端。
    ///
    /// 捕获当前槽位状态并发送完整的更新数据包。
    fn sync_state(&mut self) {
        let behaviour = self.get_behaviour_mut();
        let mut previous_tracked_stacks = Vec::new();

        for i in 0..behaviour.slots.len() {
            let stack = behaviour.slots[i].get_cloned_stack();
            previous_tracked_stacks.push(stack.clone());
            behaviour.previous_tracked_stacks[i].set_received_stack(stack);
        }

        let cursor_stack = behaviour
            .cursor_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        behaviour
            .previous_cursor_stack
            .set_received_stack(cursor_stack.clone());

        for i in 0..behaviour.properties.len() {
            let property_val = behaviour.properties[i].get();
            behaviour.tracked_property_values[i] = property_val;
        }

        let next_revision = behaviour.next_revision();

        if let Some(sync_handler) = behaviour.sync_handler.as_ref() {
            sync_handler.update_state(
                behaviour,
                &previous_tracked_stacks,
                &cursor_stack,
                &behaviour.tracked_property_values,
                next_revision,
            );
        }
    }

    /// 添加槽位和属性变化的监听器。
    fn add_listener(&mut self, listener: Arc<dyn ScreenHandlerListener>) {
        self.get_behaviour_mut().listeners.push(listener);
        self.send_content_updates();
    }

    /// 附加同步处理器并执行初始同步。
    fn update_sync_handler(&mut self, sync_handler: Arc<SyncHandler>) {
        let behaviour = self.get_behaviour_mut();
        behaviour.sync_handler = Some(sync_handler);
        self.sync_state();
    }

    /// 向客户端发送所有更新。
    ///
    /// 更新被跟踪的槽位与属性。
    fn update_to_client(&mut self) {
        for i in 0..self.get_behaviour().slots.len() {
            let behaviour = self.get_behaviour_mut();
            let slot = behaviour.slots[i].clone();
            let stack = slot.get_cloned_stack();
            self.update_tracked_slot(i, stack);
        }

        let behaviour = self.get_behaviour_mut();
        let mut prop_vec = vec![];
        for (idx, prop) in behaviour.properties.iter_mut().enumerate() {
            let value = prop.get();
            if prop.has_changed() {
                prop_vec.push((idx, value));
            }
        }

        for (idx, value) in prop_vec {
            self.update_tracked_properties(idx as i32, value);
            self.check_property_updates(idx as i32, value);
        }

        self.sync_state();
    }

    /// 更新被跟踪的属性值。
    fn update_tracked_properties(&mut self, idx: i32, value: i32) {
        let behaviour = self.get_behaviour_mut();
        if idx <= behaviour.tracked_property_values.len() as i32 {
            behaviour.tracked_property_values[idx as usize] = value;
            for listener in &behaviour.listeners {
                listener.on_property_update(behaviour, idx as u8, value);
            }
        }
    }

    /// 检查某个属性是否需要同步到客户端。
    fn check_property_updates(&mut self, idx: i32, value: i32) {
        let behaviour = self.get_behaviour_mut();
        if !behaviour.disable_sync
            && let Some(old_value) = behaviour.tracked_property_values.get(idx as usize)
        {
            let old_value = *old_value;
            if old_value != value {
                behaviour
                    .tracked_property_values
                    .insert(idx as usize, value);
                if let Some(ref sync_handler) = behaviour.sync_handler {
                    sync_handler.update_property(behaviour, idx, value);
                }
            }
        }
    }

    /// 更新槽位的跟踪状态。
    fn update_tracked_slot(&mut self, slot: usize, stack: ItemStack) {
        let behaviour = self.get_behaviour_mut();
        let other_stack = &behaviour.tracked_stacks[slot];
        if !other_stack.are_equal(&stack) {
            behaviour.tracked_stacks[slot] = stack.clone();

            for listener in &behaviour.listeners {
                listener.on_slot_update(behaviour, slot as u8, stack.clone());
            }
        }
    }

    /// 检查某个槽位是否需要同步到客户端。
    fn check_slot_updates(&mut self, slot: usize, stack: ItemStack) {
        let behaviour = self.get_behaviour_mut();
        if !behaviour.disable_sync {
            let prev_stack = &mut behaviour.previous_tracked_stacks[slot];

            if !prev_stack.is_in_sync(&stack) {
                prev_stack.set_received_stack(stack.clone());
                let next_revision = behaviour.next_revision();
                if let Some(sync_handler) = behaviour.sync_handler.as_ref() {
                    sync_handler.update_slot(behaviour, slot, &stack, next_revision);
                }
            }
        }
    }

    /// 检查光标物品堆是否需要同步。
    fn check_cursor_stack_updates(&mut self) {
        let behaviour = self.get_behaviour_mut();
        if !behaviour.disable_sync {
            let cursor_stack = behaviour
                .cursor_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !behaviour.previous_cursor_stack.is_in_sync(&cursor_stack) {
                behaviour
                    .previous_cursor_stack
                    .set_received_stack(cursor_stack.clone());
                if let Some(sync_handler) = behaviour.sync_handler.as_ref() {
                    sync_handler.update_cursor_stack(&cursor_stack);
                }
            }
        }
    }

    /// 将所有内容更新发送给监听器和同步处理器。
    fn send_content_updates(&mut self) {
        let slots_len = self.get_behaviour().slots.len();

        for i in 0..slots_len {
            let slot = self.get_behaviour().slots[i].clone();
            let stack = slot.get_cloned_stack();

            self.update_tracked_slot(i, stack.clone());
            self.check_slot_updates(i, stack);
        }

        self.check_cursor_stack_updates();

        let behaviour = self.get_behaviour_mut();
        let mut prop_vec = vec![];
        for (idx, prop) in behaviour.properties.iter_mut().enumerate() {
            let value = prop.get();
            if prop.has_changed() {
                prop_vec.push((idx, value));
            }
        }

        for (idx, value) in prop_vec {
            self.update_tracked_properties(idx as i32, value);
            self.check_property_updates(idx as i32, value);
        }
    }

    /// 检查槽位索引是否有效。
    fn is_slot_valid(&self, slot: i32) -> bool {
        slot == -1 || slot == -999 || (slot >= 0 && slot < self.get_behaviour().slots.len() as i32)
    }

    /// 禁用同步（用于批量操作）。
    fn disable_sync(&mut self) {
        let behaviour = self.get_behaviour_mut();
        behaviour.disable_sync = true;
    }

    /// 重新启用同步。
    fn enable_sync(&mut self) {
        let behaviour = self.get_behaviour_mut();
        behaviour.disable_sync = false;
    }

    /// 获取物品栏槽位对应的屏幕处理器槽位索引。
    fn get_slot_index(&self, inventory: &Arc<dyn Inventory>, slot: usize) -> Option<usize> {
        (0..self.get_behaviour().slots.len()).find(|&i| {
            Arc::ptr_eq(&self.get_behaviour().slots[i].get_inventory(), inventory)
                && self.get_behaviour().slots[i].get_index() == slot
        })
    }

    /// 从槽位执行快速移动（Shift 点击）。
    ///
    /// 必须由具体的屏幕处理器实现，用于定义
    /// 从特定槽位按住 Shift 点击时物品的去向。
    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack;

    /// 处理按钮点击事件（例如选择附魔、信标效果）。
    fn on_button_click(&mut self, _player: &dyn InventoryPlayer, _button_id: i32) -> bool {
        false
    }

    /// 模拟检查物品堆能否完整放入一段槽位范围（不改变任何状态）。
    /// 结果槽 shift 取出前必须用它预检：只能部分放入时就移动会
    /// 消耗全部费用但吞掉剩余结果（物品丢失）。
    fn can_fully_insert(&self, stack: &ItemStack, start: usize, end: usize) -> bool {
        let mut remaining = stack.item_count;
        for slot in &self.get_behaviour().slots[start..end] {
            if !slot.can_insert(stack) {
                continue;
            }
            let existing = slot.get_cloned_stack();
            let capacity = if existing.is_empty() {
                slot.get_max_item_count_for_stack(stack)
            } else if existing.are_items_and_components_equal(stack)
                && stack.are_items_and_components_equal(&existing)
            {
                slot.get_max_item_count_for_stack(&existing)
                    .saturating_sub(existing.item_count)
            } else {
                0
            };
            remaining = remaining.saturating_sub(capacity);
            if remaining == 0 {
                return true;
            }
        }
        false
    }

    /// 将物品插入一段槽位范围。
    ///
    /// 首先尝试与现有物品堆叠，然后填充空槽位。
    fn insert_item(
        &mut self,
        stack: &mut ItemStack,
        start_index: i32,
        end_index: i32,
        from_last: bool,
    ) -> bool {
        let mut success = false;
        let mut current_index = if from_last {
            end_index - 1
        } else {
            start_index
        };

        if stack.is_stackable() {
            while !stack.is_empty()
                && (if from_last {
                    current_index >= start_index
                } else {
                    current_index < end_index
                })
            {
                let slot = self.get_behaviour().slots[current_index as usize].clone();
                let mut slot_stack = slot.get_stack();

                if !slot_stack.is_empty() && slot_stack.are_items_and_components_equal(stack) {
                    let combined_count = slot_stack.item_count + stack.item_count;
                    let max_slot_count = slot.get_max_item_count_for_stack(&slot_stack);
                    if combined_count <= max_slot_count {
                        stack.set_count(0);
                        slot_stack.set_count(combined_count);
                        slot.set_stack(slot_stack);
                        success = true;
                    } else if slot_stack.item_count < max_slot_count {
                        stack.decrement(max_slot_count - slot_stack.item_count);
                        slot_stack.set_count(max_slot_count);
                        slot.set_stack(slot_stack);
                        success = true;
                    }
                }

                if from_last {
                    current_index -= 1;
                } else {
                    current_index += 1;
                }
            }
        }

        if !stack.is_empty() {
            if from_last {
                current_index = end_index - 1;
            } else {
                current_index = start_index;
            }

            while if from_last {
                current_index >= start_index
            } else {
                current_index < end_index
            } {
                let slot = self.get_behaviour().slots[current_index as usize].clone();
                let slot_stack = slot.get_stack();

                if slot_stack.is_empty() && slot.can_insert(stack) {
                    let max_count = slot.get_max_item_count_for_stack(stack);
                    slot.set_stack(stack.split(max_count.min(stack.item_count)));
                    slot.mark_dirty();
                    success = true;
                    break;
                }

                if from_last {
                    current_index -= 1;
                } else {
                    current_index += 1;
                }
            }
        }

        success
    }

    /// 处理槽位点击事件。
    ///
    /// 用于自定义点击处理的覆盖方法。返回 true 以阻止默认处理。
    fn handle_slot_click(
        &self,
        _player: &dyn InventoryPlayer,
        _click_type: MouseClick,
        _slot: Arc<dyn Slot>,
        _slot_stack: ItemStack,
        _cursor_stack: ItemStack,
    ) -> bool {
        // TODO: 未来收纳袋功能所需
        false
    }

    /// 取消所有客户端侧的更改并重新同步状态。
    fn cancel(&mut self) {
        self.sync_state();
    }

    /// 槽位点击处理的公共入口。
    fn on_slot_click(
        &mut self,
        slot_index: i32,
        button: i32,
        action_type: SlotActionType,
        player: &dyn InventoryPlayer,
    ) {
        self.internal_on_slot_click(slot_index, button, action_type, player);
    }

    /// 内部的槽位点击处理实现。
    ///
    /// 处理所有点击类型：拾取、快速移动、交换、丢弃、拖拽、克隆。
    #[expect(clippy::too_many_lines)]
    fn internal_on_slot_click(
        &mut self,
        slot_index: i32,
        button: i32,
        action_type: SlotActionType,
        player: &dyn InventoryPlayer,
    ) {
        if action_type == SlotActionType::PickupAll && button == 0 {
            let behavior = self.get_behaviour_mut();
            let mut cursor_stack = behavior
                .cursor_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // 用 saturating_sub 防御：若光标物品堆数量异常超过堆叠上限
            // （如外部编辑的存档），裸减法会下溢 panic。
            let mut to_pick_up = cursor_stack
                .get_max_stack_size()
                .saturating_sub(cursor_stack.item_count);

            for slot in &behavior.slots {
                if to_pick_up == 0 {
                    break;
                }

                let item_stack = slot.get_cloned_stack();
                if !item_stack.are_items_and_components_equal(&cursor_stack) {
                    continue;
                }

                if !slot.allow_modification(player) {
                    continue;
                }

                let taken_stack = slot.safe_take(
                    item_stack.item_count.min(to_pick_up),
                    cursor_stack
                        .get_max_stack_size()
                        .saturating_sub(cursor_stack.item_count),
                    player,
                );
                to_pick_up -= taken_stack.item_count;
                cursor_stack.increment(taken_stack.item_count);
            }
        } else if action_type == SlotActionType::QuickCraft {
            let drag_type = button & 3;
            let drag_button = (button >> 2) & 3;
            let behaviour = self.get_behaviour_mut();
            if drag_type == 0 {
                behaviour.drag_slots.clear();
            } else if drag_type == 1 {
                if slot_index < 0 {
                    warn!("拖拽动作的槽位索引无效：{slot_index}，必须 >= 0");
                    return;
                }
                let cursor_stack = behaviour
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);

                let slot = &behaviour.slots[slot_index as usize];
                let stack = slot.get_stack();
                if !cursor_stack.is_empty()
                    && slot.can_insert(&cursor_stack)
                    && (stack.are_items_and_components_equal(&cursor_stack) || stack.is_empty())
                    && slot.get_max_item_count_for_stack(&stack) > stack.item_count
                {
                    behaviour.drag_slots.push(slot_index as u32);
                }
            } else if drag_type == 2 && !behaviour.drag_slots.is_empty() {
                // 处理拖拽结束
                if behaviour.drag_slots.len() == 1 {
                    let slot = behaviour.drag_slots[0] as i32;
                    behaviour.drag_slots.clear();
                    self.internal_on_slot_click(slot, drag_button, SlotActionType::Pickup, player);

                    return;
                }
                if drag_button == 2 && !player.has_infinite_materials() {
                    return; // 仅创造模式
                }

                let mut cursor_stack = behaviour
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let initial_count = cursor_stack.item_count;
                let slots_count = behaviour.drag_slots.len();
                for slot_index in &behaviour.drag_slots {
                    let Some(slot) = behaviour.slots.get(*slot_index as usize).cloned() else {
                        continue;
                    };
                    let stack = slot.get_stack();

                    if (stack.are_items_and_components_equal(&cursor_stack) || stack.is_empty())
                        && slot.can_insert(&cursor_stack)
                    {
                        let mut inserting_count = match drag_button {
                            0 => (initial_count as usize)
                                .checked_div(slots_count)
                                .map_or(0, |c| c as u8),
                            1 => 1,
                            2 => {
                                cursor_stack.item_count = cursor_stack.get_max_stack_size();
                                cursor_stack.item_count
                            }
                            _ => 0,
                        };
                        inserting_count = inserting_count
                            // saturating_sub：槽位因存档损坏出现超堆叠
                            // 时 u8 裸减会下溢（debug panic/release 回绕）
                            .min(
                                slot.get_max_item_count_for_stack(&stack)
                                    .saturating_sub(stack.item_count),
                            )
                            .min(cursor_stack.item_count);
                        if inserting_count > 0 {
                            let mut new_stack = stack.clone();
                            if new_stack.is_empty() {
                                new_stack = cursor_stack.copy_with_count(0);
                            }
                            new_stack.increment(inserting_count);
                            slot.set_stack(new_stack);
                            if drag_button != 2 {
                                cursor_stack.decrement(inserting_count);
                            }
                            if cursor_stack.is_empty() {
                                *cursor_stack = ItemStack::EMPTY.clone();
                                break;
                            }
                        }
                    }
                }

                if drag_button == 2 {
                    *cursor_stack = ItemStack::EMPTY.clone();
                }
                behaviour.drag_slots.clear();
            }
        } else if action_type == SlotActionType::Throw {
            if slot_index >= 0
                && self
                    .get_behaviour()
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_empty()
            {
                let slot = self.get_behaviour().slots[slot_index as usize].clone();
                let prev_stack = slot.get_cloned_stack();
                if !prev_stack.is_empty() {
                    // 原版 THROW 语义：单次取出（Ctrl+Q 整组、Q 一个），
                    // on_take_item 由 safe_take 内部触发一次。
                    // 注意不能循环取：合成结果槽的缓存不随取出变化，
                    // 循环条件会永远成立——无限刷物品并挂死处理线程；
                    // 也不能在 safe_take 之外再调 on_take_item，否则
                    // 原料/经验会被重复消耗。
                    let take_count = if button == 1 {
                        prev_stack.item_count
                    } else {
                        1
                    };
                    let drop_stack = slot.safe_take(take_count, u8::MAX, player);
                    if !drop_stack.is_empty() {
                        player.drop_item(drop_stack, true);
                    }
                }
            }
        } else if action_type == SlotActionType::Clone {
            if player.has_infinite_materials() && slot_index >= 0 {
                let behaviour = self.get_behaviour_mut();
                let mut cursor_stack = behaviour
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !cursor_stack.is_empty() {
                    return;
                }
                let slot = behaviour.slots[slot_index as usize].clone();
                let stack = slot.get_stack();
                *cursor_stack = stack.copy_with_count(stack.get_max_stack_size());
            }
        } else if (action_type == SlotActionType::Pickup
            || action_type == SlotActionType::QuickMove)
            && (button == 0 || button == 1)
        {
            let click_type = if button == 0 {
                MouseClick::Left
            } else {
                MouseClick::Right
            };

            // 若物品在背包外则丢弃
            if slot_index == SLOT_INDEX_OUTSIDE {
                let mut cursor_stack = self
                    .get_behaviour()
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !cursor_stack.is_empty() {
                    if click_type == MouseClick::Left {
                        player.drop_item(cursor_stack.clone(), true);
                        *cursor_stack = ItemStack::EMPTY.clone();
                    } else {
                        player.drop_item(cursor_stack.split(1), true);
                    }
                }
            } else if action_type == SlotActionType::QuickMove {
                if slot_index < 0 {
                    return;
                }

                let slot = self.get_behaviour().slots[slot_index as usize].clone();

                if !slot.can_take_items(player) {
                    return;
                }

                let mut moved_stack = self.quick_move(player, slot_index);

                while !moved_stack.is_empty()
                    && ItemStack::are_items_and_components_equal(
                        &slot.get_cloned_stack(),
                        &moved_stack,
                    )
                {
                    moved_stack = self.quick_move(player, slot_index);
                }
            } else {
                // 拾取
                if slot_index < 0 {
                    return;
                }

                // 只有玩家物品栏界面（无窗口类型）的 5..=8 槽位是盔甲槽；
                // 通用容器窗口的同索引是普通格子，不能触发装备更换
                let has_equipment_slots = self.get_behaviour().window_type.is_none();

                let slot = self.get_behaviour().slots[slot_index as usize].clone();

                if click_type == MouseClick::Left {
                    slot.on_click(player);
                }

                let slot_stack = slot.get_cloned_stack();
                let mut cursor_stack = self
                    .get_behaviour()
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);

                if click_type == MouseClick::Right {
                    let mut intercepted = false;

                    if !cursor_stack.is_empty() {
                        let mut inner_slot_stack = slot.get_stack();
                        if let Some(bundle) = inner_slot_stack.get_data_component_mut::<papokin_data::data_component_impl::BundleContentsImpl>()
                            && bundle.try_insert(&mut cursor_stack) {
                                slot.set_stack(inner_slot_stack);
                                intercepted = true;
                            }
                    }

                    if !intercepted && !slot_stack.is_empty()
                        && let Some(bundle) = cursor_stack.get_data_component_mut::<papokin_data::data_component_impl::BundleContentsImpl>() {
                            let mut inner_slot_stack = slot.get_stack();
                            if bundle.try_insert(&mut inner_slot_stack) {
                                if inner_slot_stack.item_count == 0 {
                                    inner_slot_stack = ItemStack::EMPTY.clone();
                                }
                                slot.set_stack(inner_slot_stack);
                                intercepted = true;
                            }
                        }

                    if !intercepted && cursor_stack.is_empty() {
                        let mut inner_slot_stack = slot.get_stack();
                        if let Some(bundle) = inner_slot_stack.get_data_component_mut::<papokin_data::data_component_impl::BundleContentsImpl>()
                            && let Some(extracted) = bundle.try_extract() {
                                *cursor_stack = extracted;
                                slot.set_stack(inner_slot_stack);
                                intercepted = true;
                            }
                    }

                    if !intercepted && slot_stack.is_empty()
                        && let Some(bundle) = cursor_stack.get_data_component_mut::<papokin_data::data_component_impl::BundleContentsImpl>()
                        && let Some(extracted) = bundle.try_extract() {
                            slot.set_stack(extracted);
                            intercepted = true;
                        }

                    if intercepted {
                        if cursor_stack.item_count == 0 {
                            *cursor_stack = ItemStack::EMPTY.clone();
                        }
                        slot.mark_dirty();
                        return;
                    }
                }

                let equipment_slot = cursor_stack
                    .get_data_component::<EquippableImpl>()
                    .map_or(&EquipmentSlot::MAIN_HAND, |equippable| equippable.slot);

                if self.handle_slot_click(
                    player,
                    click_type.clone(),
                    slot.clone(),
                    slot_stack.clone(),
                    cursor_stack.clone(),
                ) {
                    return;
                }

                if slot_stack.is_empty() {
                    if !cursor_stack.is_empty() {
                        if has_equipment_slots
                            && equipment_slot.slot_type() == EquipmentType::HumanoidArmor
                            && (5..9).contains(&slot_index)
                        {
                            player.enqueue_equipment_change(equipment_slot, &cursor_stack);
                        }

                        let transfer_count = if click_type == MouseClick::Left {
                            cursor_stack.item_count
                        } else {
                            1
                        };
                        *cursor_stack =
                            slot.insert_stack_count(cursor_stack.clone(), transfer_count);
                    }
                } else if slot.can_take_items(player) {
                    if cursor_stack.is_empty() {
                        let take_count = if click_type == MouseClick::Left {
                            slot_stack.item_count
                        } else {
                            slot_stack.item_count.div_ceil(2)
                        };
                        let taken = slot.try_take_stack_range(take_count, u8::MAX, player);
                        if let Some(taken) = taken {
                            // 反转操作顺序，不应影响任何结果
                            *cursor_stack = taken.clone();
                            slot.on_take_item(player, &taken);

                            if has_equipment_slots && (5..9).contains(&slot_index) {
                                let equipment_slot = cursor_stack
                                    .get_data_component::<EquippableImpl>()
                                    .map_or(&EquipmentSlot::MAIN_HAND, |equippable| {
                                        equippable.slot
                                    });
                                player.enqueue_equipment_change(equipment_slot, ItemStack::EMPTY);
                            }
                        }
                    } else if slot.can_insert(&cursor_stack) {
                        if has_equipment_slots
                            && equipment_slot.slot_type() == EquipmentType::HumanoidArmor
                            && (5..9).contains(&slot_index)
                        {
                            player.enqueue_equipment_change(equipment_slot, &cursor_stack);
                        }

                        if ItemStack::are_items_and_components_equal(&slot_stack, &cursor_stack) {
                            let insert_count = if click_type == MouseClick::Left {
                                cursor_stack.item_count
                            } else {
                                1
                            };
                            *cursor_stack =
                                slot.insert_stack_count(cursor_stack.clone(), insert_count);
                        } else if cursor_stack.item_count
                            <= slot.get_max_item_count_for_stack(&cursor_stack)
                        {
                            let old_cursor_stack = cursor_stack.clone();
                            *cursor_stack = slot_stack;
                            slot.set_stack(old_cursor_stack);
                        }
                    } else if ItemStack::are_items_and_components_equal(&slot_stack, &cursor_stack)
                    {
                        let taken = slot.try_take_stack_range(
                            slot_stack.item_count,
                            cursor_stack
                                .get_max_stack_size()
                                .saturating_sub(cursor_stack.item_count),
                            player,
                        );

                        if let Some(taken) = taken {
                            cursor_stack.increment(taken.item_count);
                            slot.on_take_item(player, &taken);
                        }
                    }
                }

                slot.mark_dirty();
            }
        } else if action_type == SlotActionType::Swap && ((0..9).contains(&button) || button == 40)
        {
            if slot_index < 0 {
                return;
            }
            let player_inventory = player.get_inventory();
            let mut button_stack = player_inventory.get_stack(button as usize);
            let source_slot = self.get_behaviour().slots[slot_index as usize].clone();
            let source_stack = source_slot.get_cloned_stack();

            if !button_stack.is_empty() || !source_stack.is_empty() {
                if button_stack.is_empty() {
                    if source_slot.can_take_items(player) {
                        player_inventory.set_stack(button as usize, source_stack.clone());
                        source_slot.set_stack(ItemStack::EMPTY.clone());
                        source_slot.on_take_item(player, &source_stack);
                    }
                } else if source_stack.is_empty() && source_slot.can_insert(&button_stack) {
                    let max_count = source_slot.get_max_item_count_for_stack(&button_stack);
                    if button_stack.item_count > max_count {
                        source_slot.set_stack(button_stack.split(max_count));
                        player_inventory.set_stack(button as usize, button_stack);
                    } else {
                        player_inventory.set_stack(button as usize, ItemStack::EMPTY.clone());
                        source_slot.set_stack(button_stack);
                    }
                } else if source_slot.can_take_items(player)
                    && source_slot.can_insert(&button_stack)
                {
                    let max_count = source_slot.get_max_item_count_for_stack(&button_stack);
                    if button_stack.item_count > max_count {
                        source_slot.set_stack(button_stack.split(max_count));
                        player_inventory.set_stack(button as usize, button_stack);
                        source_slot.on_take_item(player, &source_stack);

                        let mut displaced_stack = source_stack;
                        player_inventory.insert_stack_anywhere(&mut displaced_stack);
                        if !displaced_stack.is_empty() {
                            player.drop_item(displaced_stack, true);
                        }
                    } else {
                        player_inventory.set_stack(button as usize, source_stack.clone());
                        source_slot.set_stack(button_stack);
                        source_slot.on_take_item(player, &source_stack);
                    }
                }
            }
        }
    }
}

pub trait ScreenHandlerListener: Send + Sync {
    fn on_slot_update(
        &self,
        _screen_handler: &ScreenHandlerBehaviour,
        _slot: u8,
        _stack: ItemStack,
    ) {
    }
    fn on_property_update(
        &self,
        _screen_handler: &ScreenHandlerBehaviour,
        _property: u8,
        _value: i32,
    ) {
    }
}

pub type SharedScreenHandler = Arc<Mutex<dyn ScreenHandler>>;

pub trait ScreenHandlerFactory: Send + Sync {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler>;
    fn get_display_name(&self) -> TextComponent;
}

pub struct ScreenHandlerBehaviour {
    /// 此屏幕处理器中的槽位（同时包含容器槽位与玩家槽位）。
    pub slots: Vec<Arc<dyn Slot>>,
    /// 用于客户端与服务器匹配的同步 ID（与协议中的窗口 ID 对应）。
    pub sync_id: u8,
    /// 已注册的槽位/属性变更监听器。
    pub listeners: Vec<Arc<dyn ScreenHandlerListener>>,
    /// 用于向客户端发送更新的同步处理器。
    pub sync_handler: Option<Arc<SyncHandler>>,
    /// 当前跟踪的物品堆，用于与之前的状态进行比较。
    //TODO: 检查这是否必要
    pub tracked_stacks: Vec<ItemStack>,
    /// 玩家光标当前持有的物品（光标物品）。
    pub cursor_stack: Arc<Mutex<ItemStack>>,
    /// 用于检测需要同步的更改的先前已跟踪物品堆。
    pub previous_tracked_stacks: Vec<TrackedStack>,
    /// 用于检测光标变化的先前光标物品堆。
    pub previous_cursor_stack: TrackedStack,
    /// 用于同步跟踪的修订计数器（每次更改时递增）。
    pub revision: AtomicU32,
    /// 同步是否被暂时禁用（用于批量操作）。
    pub disable_sync: bool,
    /// 容器属性（熔炉进度、附魔等级等）。
    pub properties: Vec<ScreenProperty>,
    /// 用于检测变化的已跟踪属性值。
    pub tracked_property_values: Vec<i32>,
    /// 此容器的窗口类型（决定客户端 UI）。
    pub window_type: Option<WindowType>,
    /// 拖拽操作中选中的槽位（用于多槽位分配）。
    pub drag_slots: Vec<u32>,
    /// 玩家能否从物品栏中取出物品。
    pub allow_grab_items: bool,
    /// 玩家能否将自己的物品放入该物品栏。
    pub allow_put_items: bool,
    /// 属于容器的槽位数（不含玩家物品栏）。
    pub container_slots: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickType {
    Left,
    Right,
    ShiftLeft,
    ShiftRight,
    Middle,
    Drop,
    ControlDrop,
    DoubleClick,
    NumberKey(u8),
    Unknown,
}

impl ScreenHandlerBehaviour {
    #[must_use]
    pub fn new(sync_id: u8, window_type: Option<WindowType>) -> Self {
        Self {
            slots: Vec::new(),
            sync_id,
            listeners: Vec::new(),
            sync_handler: None,
            tracked_stacks: Vec::new(),
            cursor_stack: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            previous_tracked_stacks: Vec::new(),
            previous_cursor_stack: TrackedStack::EMPTY,
            revision: AtomicU32::new(0),
            disable_sync: false,
            properties: Vec::new(),
            tracked_property_values: Vec::new(),
            window_type,
            drag_slots: Vec::new(),
            allow_grab_items: true,
            allow_put_items: true,
            container_slots: 0,
        }
    }

    pub fn next_revision(&self) -> u32 {
        self.revision.fetch_add(1, Ordering::Relaxed);
        self.revision.fetch_and(32767, Ordering::Relaxed) & 32767
    }
}
