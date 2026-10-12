//! 大容量箱族的惰性物品槽存储。
//!
//! 空容器（含未开封的战利品箱——`LootTable` 待定期间物品永不
//! 读取，以及玩家放置后尚未存放物品的箱子）在 `None` 态零堆
//! 分配；首次写入物化定长槽数组。读取路径一律不物化。
//!
//! 语义等价性：对全部读接口，`None` 态与「全空物品堆数组」
//! 逐点等价（`get_stack` 回落空堆、`is_empty` 为真、NBT 写出
//! 均不产出 `Items` 键——`sync_write_items_to_nbt` 对全空数组
//! 本就不写键）。写接口物化后行为与常驻数组一致。

use std::sync::RwLock;

use papokin_data::item_stack::ItemStack;
use papokin_nbt::compound::NbtCompound;

use crate::sync_write_items_to_nbt;

/// 惰性定长物品槽存储。
pub struct LazyItemSlots<const N: usize> {
    slots: RwLock<Option<Box<[ItemStack; N]>>>,
}

impl<const N: usize> Default for LazyItemSlots<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> LazyItemSlots<N> {
    /// 空态（未物化）新存储。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: RwLock::new(None),
        }
    }

    /// 读取闭包视图；未物化时回调收到 `None`（读路径不物化）。
    pub fn with_read<R>(&self, f: impl FnOnce(Option<&[ItemStack; N]>) -> R) -> R {
        let guard = self
            .slots
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(guard.as_deref())
    }

    /// 写闭包视图；未物化时先物化全空数组再进入回调。
    pub fn with_write<R>(&self, f: impl FnOnce(&mut [ItemStack; N]) -> R) -> R {
        let mut guard = self
            .slots
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slots = guard
            .get_or_insert_with(|| Box::new(std::array::from_fn(|_| ItemStack::EMPTY.clone())));
        f(slots)
    }

    /// 全部槽位是否为空（未物化即空）。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.with_read(|slots| slots.is_none_or(|a| a.iter().all(ItemStack::is_empty)))
    }

    /// 单槽读取；未物化（或槽位越界）返回空物品堆。
    #[must_use]
    pub fn get_stack(&self, slot: usize) -> ItemStack {
        self.with_read(|slots| {
            slots.map_or_else(
                || ItemStack::EMPTY.clone(),
                |a| {
                    a.get(slot)
                        .map_or_else(|| ItemStack::EMPTY.clone(), ItemStack::clone)
                },
            )
        })
    }

    /// 借用式单槽读取回调；未物化时以静态空堆进入回调（零分配）。
    pub fn with_stack(&self, slot: usize, f: &mut dyn FnMut(&ItemStack)) {
        self.with_read(|slots| match slots.and_then(|a| a.get(slot)) {
            Some(stack) => f(stack),
            None => f(ItemStack::EMPTY),
        });
    }

    /// 锁稳定单槽 RMW：未物化时对临时空堆执行回调，回调报告
    /// 已修改才物化并提交（`false` 不得改动的硬约不变——
    /// 不合规调用方在旧形态下同样无契约保证）。
    pub fn with_stack_mut(&self, slot: usize, f: &mut dyn FnMut(&mut ItemStack) -> bool) -> bool {
        let mut staged = self.get_stack(slot);
        if !f(&mut staged) {
            return false;
        }
        self.with_write(|a| a[slot] = staged);
        true
    }

    /// 从 NBT 填充：先解析到临时缓冲，仅当至少一件物品时物化
    /// （空 `Items` 列表保持 `None`——与全空数组的既有语义一致）。
    pub fn fill_from_nbt(&self, nbt: &NbtCompound) {
        let Some(entries) = parse_items(nbt) else {
            return;
        };
        if entries.is_empty() {
            return;
        }
        let mut guard = self
            .slots
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let array = guard
            .get_or_insert_with(|| Box::new(std::array::from_fn(|_| ItemStack::EMPTY.clone())));
        for (slot, stack) in entries {
            if slot < N {
                array[slot] = stack;
            }
        }
    }

    /// 写出 `Items` 键到 NBT；未物化时不写键（与全空数组输出
    /// 逐字节一致）。
    pub fn write_to_nbt(&self, nbt: &mut NbtCompound) {
        self.with_read(|slots| {
            if let Some(items) = slots {
                sync_write_items_to_nbt(items, nbt);
            }
        });
    }

    /// 非阻塞写出（区块数据发送路径沿用 `try_read` 语义）：
    /// 锁被持有时跳过并返回 `false`。
    pub fn try_write_to_nbt(&self, nbt: &mut NbtCompound) -> bool {
        let Ok(guard) = self.slots.try_read() else {
            return false;
        };
        if let Some(items) = guard.as_deref() {
            sync_write_items_to_nbt(items, nbt);
        }
        true
    }

    /// 清空全部槽位并回到未物化态（可观察状态与全空数组一致，
    /// 同时释放槽数组驻留）。
    pub fn clear(&self) {
        let mut guard = self
            .slots
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = None;
    }
}

/// 解析 NBT 的 `Items` 列表到 `(槽位, 物品堆)` 对；无键时
/// 返回 `None`（与 `sync_read_items_from_nbt` 的逐条语义对齐）。
fn parse_items(nbt: &NbtCompound) -> Option<Vec<(usize, ItemStack)>> {
    let inventory_list = nbt.get_list("Items")?;
    let mut entries = Vec::new();
    for tag in inventory_list {
        if let Some(item_compound) = tag.extract_compound()
            && let Some(slot_byte) = item_compound.get_byte("Slot")
            && let Some(item_stack) = ItemStack::read_item_stack(item_compound)
        {
            entries.push((slot_byte as usize, item_stack));
        }
    }
    Some(entries)
}
