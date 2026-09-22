//! 类熔炉槽位实现。
//!
//! 此模块为熔炉类容器提供专用的槽位类型。
//! 熔炉拥有三个行为特殊的槽位：
//! - 输入槽（上方）：接受任何可熔炼物品
//! - 燃料槽（下方）：只接受燃料物品（煤炭、木炭等）
//! - 输出槽：不能接收物品，取出物品时会给予经验

use std::sync::{Arc, atomic::AtomicU8};

use crate::{inventory::Inventory, window_property::ExperienceContainer};
use papokin_data::{fuels::is_fuel, item::Item, statistic::StatisticCategory};

use tracing::debug;

use crate::{screen_handler::InventoryPlayer, slot::Slot};

/// 熔炉槽位类型。
#[derive(Debug, Clone, Copy)]
pub enum FurnaceLikeSlotType {
    /// 输入槽（顶部）——接受待熔炼的物品。
    Top = 0,
    /// 燃料槽（底部）- 接受燃料物品。
    Bottom = 1,
}

/// 熔炉输入或燃料槽位。
///
/// 输入槽接受任意物品，而燃料槽只接受
/// 有效燃料物品（以及用作岩浆燃料的空桶）。
pub struct FurnaceLikeSlot {
    pub inventory: Arc<dyn Inventory>,
    pub slot_type: FurnaceLikeSlotType,
    pub index: usize,
    pub id: AtomicU8,
}

impl FurnaceLikeSlot {
    /// 创建新的熔炉槽位。
    ///
    /// # Arguments
    /// - `inventory` - 熔炉的物品栏
    /// - `slot_type` - 表示这是输入槽（上方）还是燃料槽（下方）
    pub fn new(inventory: Arc<dyn Inventory>, slot_type: FurnaceLikeSlotType) -> Self {
        Self {
            inventory,
            slot_type,
            index: slot_type as usize,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for FurnaceLikeSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id
            .store(id as u8, std::sync::atomic::Ordering::Relaxed);
    }

    /// 根据槽位类型限制插入操作。
    ///
    /// - 顶部槽位：接受任意物品（可熔炼物）
    /// - 底部槽位：只接受燃料物品和桶
    fn can_insert(&self, stack: &papokin_data::item_stack::ItemStack) -> bool {
        match self.slot_type {
            FurnaceLikeSlotType::Top => true,
            FurnaceLikeSlotType::Bottom => {
                is_fuel(stack.item.id) || stack.item.id == Item::BUCKET.id
            }
        }
    }

    fn get_max_item_count_for_stack(&self, stack: &papokin_data::item_stack::ItemStack) -> u8 {
        match self.slot_type {
            FurnaceLikeSlotType::Bottom if stack.item.id == Item::BUCKET.id => 1,
            _ => self.get_max_item_count().min(stack.get_max_stack_size()),
        }
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

/// 类熔炉容器的输出槽位。
///
/// 此槽位不能直接接收物品（物品通过熔炼放入此处）。
/// 当从此槽位取出物品时，玩家会获得经验
/// 基于所使用的熔炼配方。
pub struct FurnaceOutputSlot {
    pub inventory: Arc<dyn Inventory>,
    pub experience_container: Arc<dyn ExperienceContainer>,
    pub id: AtomicU8,
}

impl FurnaceOutputSlot {
    /// 创建新的熔炉输出槽位。
    ///
    /// # Arguments
    /// - `inventory` - 熔炉的物品栏
    /// - `experience_container` - 跟踪累积经验的容器
    pub fn new(
        inventory: Arc<dyn Inventory>,
        experience_container: Arc<dyn ExperienceContainer>,
    ) -> Self {
        Self {
            inventory,
            experience_container,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for FurnaceOutputSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        2 // 输出槽位始终为索引 2
    }

    fn set_id(&self, id: usize) {
        self.id
            .store(id as u8, std::sync::atomic::Ordering::Relaxed);
    }

    /// 从此槽位取出物品时给予经验。
    fn on_take_item(
        &self,
        player: &dyn InventoryPlayer,
        stack: &papokin_data::item_stack::ItemStack,
    ) {
        debug!("FurnaceOutputSlot：on_take_item 被调用");
        player.increment_stat(
            StatisticCategory::Crafted,
            stack.item.id as i32,
            stack.item_count as i32,
        );
        // 提取累积的经验并授予玩家
        let experience = self.experience_container.extract_experience();
        debug!("FurnaceOutputSlot：提取的经验值 = {experience}");
        if experience > 0 {
            debug!("FurnaceOutputSlot：向玩家发放 {experience} 点经验");
            player.award_experience(experience);
        }
        self.mark_dirty();
    }

    /// 输出槽位无法接收插入的物品。
    fn can_insert(&self, _stack: &papokin_data::item_stack::ItemStack) -> bool {
        // 无法向输出槽插入物品
        false
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}
