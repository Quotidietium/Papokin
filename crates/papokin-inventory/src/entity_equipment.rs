//! 实体装备管理。
//!
//! 本模块处理实体装备槽位的存储与管理，
//! 例如盔甲（头部、胸部、腿部、脚部）与副手物品。
//!
//! 装备与主物品栏分开存储，并显示在
//! 实体模型（盔甲渲染在玩家身上，手持物品在
//! 手中可见）。

use std::collections::HashMap;

use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item_stack::ItemStack;

/// 实体的装备存储。
///
/// 存储装备于盔甲槽位的物品（头盔、胸甲、护腿、靴子），以及
/// 副手槽位。装备独立于主物品栏
/// 并影响实体的外观和数值。
///
/// 另请参见：[`EquipmentSlot`](EquipmentSlot)
// EntityEquipment.java
#[derive(Clone, Default)]
pub struct EntityEquipment {
    /// 装备栏位到所装备物品的映射。
    ///
    /// 键为装备槽类型（头部、胸部、腿部、脚部、副手）。
    pub equipment: HashMap<EquipmentSlot, ItemStack>,
}

impl EntityEquipment {
    /// 创建新的空装备存储。
    #[must_use]
    pub fn new() -> Self {
        Self {
            equipment: HashMap::new(),
        }
    }

    /// 在槽位中装备一件物品，返回之前的物品。
    ///
    /// # Arguments
    /// - `slot` - 装备槽位
    /// - `stack` - 要装备的物品
    ///
    /// # Returns
    /// 之前装备的物品；如果槽位为空，则为空物品堆。
    pub fn put(&mut self, slot: &EquipmentSlot, stack: ItemStack) -> ItemStack {
        self.equipment
            .insert(slot.clone(), stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// 获取槽位中的物品。
    ///
    /// # Returns
    /// 已装备的物品，若未装备任何物品则为空堆栈。
    #[must_use]
    pub fn get(&self, slot: &EquipmentSlot) -> ItemStack {
        self.equipment
            .get(slot)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// 检查所有装备槽位是否为空。
    pub fn is_empty(&self) -> bool {
        self.equipment.values().all(ItemStack::is_empty)
    }

    /// 清空所有已装备的物品。
    pub fn clear(&mut self) {
        self.equipment.clear();
    }

    // TODO: 刻运算 - 装备更新、耐久损耗等。
}
