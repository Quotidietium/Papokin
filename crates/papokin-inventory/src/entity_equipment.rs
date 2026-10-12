//! 实体装备管理。
//!
//! 本模块处理实体装备槽位的存储与管理，
//! 例如盔甲（头部、胸部、腿部、脚部）与副手物品。
//!
//! 装备与主物品栏分开存储，并显示在
//! 实体模型（盔甲渲染在玩家身上，手持物品在
//! 手中可见）。

use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item_stack::ItemStack;

/// 判别值到规范槽位实例的映射表。
///
/// 键空间不变式：装备键在全仓恒出自 8 个 `const` 规范实例
/// （`EquippableImpl.slot` 为 `&'static`，`get_from_name` 返回
/// 静态表，寻路/掉落等全部传引用 const），同一判别值不可能
/// 对应第二种数据形态——判别值索引因此是完备键。
///
/// 用 `static` 而非 `const`：迭代访问器以运行时下标取引用，
/// `const` 无法把该引用晋升为 `'static`。
static SLOT_TABLE: [EquipmentSlot; 8] = [
    EquipmentSlot::MAIN_HAND,
    EquipmentSlot::OFF_HAND,
    EquipmentSlot::FEET,
    EquipmentSlot::LEGS,
    EquipmentSlot::CHEST,
    EquipmentSlot::HEAD,
    EquipmentSlot::BODY,
    EquipmentSlot::SADDLE,
];

/// 实体的装备存储。
///
/// 存储装备于盔甲槽位的物品（头盔、胸甲、护腿、靴子），以及
/// 副手槽位。装备独立于主物品栏
/// 并影响实体的外观和数值。
///
/// 载体为判别值直索引的紧凑数组：未装备任何物品时零堆分配
/// （`None` 一层即空态），装第一件物品时才分配 8 槽数组。
/// 迭代按判别值固定序产出（原 `HashMap` 迭代序本就随进程
/// 种子不定，消费方均按槽位自带索引/独立结算解耦，无序契约）。
///
/// 另请参见：[`EquipmentSlot`](EquipmentSlot)
// EntityEquipment.java
#[derive(Clone, Default)]
pub struct EntityEquipment {
    /// 装备槽判别值索引的存储；`None` 表示从未装备过任何物品。
    slots: Option<Box<[Option<ItemStack>; 8]>>,
}

impl EntityEquipment {
    /// 创建新的空装备存储。
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: None }
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
        let idx = slot.discriminant() as usize;
        let slots = self
            .slots
            .get_or_insert_with(|| Box::new(std::array::from_fn(|_| None)));
        slots[idx]
            .replace(stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// 获取槽位中的物品。
    ///
    /// # Returns
    /// 已装备的物品，若未装备任何物品则为空堆栈。
    #[must_use]
    pub fn get(&self, slot: &EquipmentSlot) -> ItemStack {
        self.get_opt(slot)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// 获取槽位中物品的引用；未装备时为 `None`。
    ///
    /// 对齐旧 `HashMap::get` 的直接访问语义（调用方自行
    /// 处理空态，不做空堆栈回落）。
    #[must_use]
    pub fn get_opt(&self, slot: &EquipmentSlot) -> Option<&ItemStack> {
        let idx = slot.discriminant() as usize;
        self.slots.as_ref()?.get(idx)?.as_ref()
    }

    /// 可变获取槽位中的物品；未装备（或槽位从未写入）时为 `None`。
    ///
    /// 对齐旧 `HashMap::get_mut` 的直接访问语义。
    pub fn get_mut(&mut self, slot: &EquipmentSlot) -> Option<&mut ItemStack> {
        let idx = slot.discriminant() as usize;
        self.slots.as_mut()?.get_mut(idx)?.as_mut()
    }

    /// 迭代已写入的槽位与物品（判别值固定序）。
    ///
    /// 对齐旧 `HashMap` 迭代：显式写入过空物品堆的槽位同样
    /// 产出（与既有语义一致，消费方自行过滤）。
    pub fn iter_present(&self) -> impl Iterator<Item = (&'static EquipmentSlot, &ItemStack)> {
        self.slots
            .as_deref()
            .into_iter()
            .flat_map(|slots| slots.iter().enumerate())
            .filter_map(|(idx, stack)| stack.as_ref().map(|stack| (&SLOT_TABLE[idx], stack)))
    }

    /// 检查所有装备槽位是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots
            .as_ref()
            .is_none_or(|slots| slots.iter().flatten().all(ItemStack::is_empty))
    }

    /// 可变迭代已写入槽位的物品（判别值固定序）。
    ///
    /// 对齐旧 `HashMap::values_mut` 的直接访问语义。
    pub fn iter_present_mut(&mut self) -> impl Iterator<Item = &mut ItemStack> {
        self.slots.as_deref_mut().into_iter().flatten().flatten()
    }

    /// 可变迭代已写入的槽位与物品（判别值固定序）。
    ///
    /// 对齐旧 `&mut HashMap` 迭代（键值都可变访问；键侧
    /// 以规范静态实例替代原 map 键的引用）。
    pub fn iter_slots_mut(
        &mut self,
    ) -> impl Iterator<Item = (&'static EquipmentSlot, &mut ItemStack)> {
        self.slots
            .as_deref_mut()
            .into_iter()
            .flat_map(|slots| slots.iter_mut().enumerate())
            .filter_map(|(idx, slot)| slot.as_mut().map(|stack| (&SLOT_TABLE[idx], stack)))
    }

    /// 清空所有已装备的物品。
    pub fn clear(&mut self) {
        self.slots = None;
    }

    /// 移除槽位中的物品并返回（槽位未写入过则为 `None`）。
    ///
    /// 对齐旧 `HashMap::remove` 的直接访问语义；数组保留
    /// 不缩容（与旧表 remove 后保留桶一致，空态仍由
    /// [`Self::is_empty`] 判定）。
    pub fn remove(&mut self, slot: &EquipmentSlot) -> Option<ItemStack> {
        let idx = slot.discriminant() as usize;
        self.slots.as_mut()?.get_mut(idx)?.take()
    }

    // TODO: 刻运算 - 装备更新、耐久损耗等。
}
