use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_nbt::{compound::NbtCompound, tag::NbtTag};
use std::any::Any;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

/// 物品栏实例的点击串行化身份键（`Arc` 数据指针地址）。
#[must_use]
pub fn inventory_lock_id(inventory: &Arc<dyn Inventory>) -> usize {
    Arc::as_ptr(inventory).cast::<()>() as usize
}

pub trait Inventory: Send + Sync + Clearable {
    /// 收集本物品栏参与「点击串行化」的身份键。
    ///
    /// 不同玩家的屏幕处理器互斥锁互不相同，对同一共享容器（箱子、
    /// 木桶等）的读-改-写可以交错：陈旧快照整体写回即复制/丢失物
    /// 品。`ScreenHandler::on_slot_click` 会收集本屏涉及的全部身份
    /// 键并映射到全局条带锁，跨玩家串行化整次点击。
    ///
    /// 默认实现以调用方传入的 `self_id`（[`inventory_lock_id`]）为
    /// 键，共享方块实体等稳定实例直接适用；组合物品栏（如双箱的
    /// [`crate::inventory::DoubleInventory`]）应覆写并递归到底层，
    /// 使不同玩家打开的临时包装实例仍解析到同一组键。
    fn collect_click_lock_ids(&self, self_id: usize, out: &mut Vec<usize>) {
        out.push(self_id);
    }

    fn size(&self) -> usize;

    fn is_empty(&self) -> bool;

    /// 以借用方式访问槽位物品堆（锁内回调，零克隆读取）。
    ///
    /// 高频全屏扫描（`send_content_updates` 等）的比较-先行预检
    /// 用它避免未变化槽位的读取克隆——`ItemStack` 组件补丁的
    /// 深克隆只在确认变化后发生。覆写实现在读锁内回调。
    ///
    /// 语义约束：回调内**不得**调用同一物品栏的任何方法（覆写
    /// 方在持锁状态下回调，重入可能死锁），只做纯读比较/浅访问。
    ///
    /// 默认实现回退为 [`Inventory::get_stack`] 克隆后借出（功能
    /// 等价，仅失去零克隆收益）。
    fn with_stack(&self, slot: usize, f: &mut dyn FnMut(&ItemStack)) {
        let stack = self.get_stack(slot);
        f(&stack);
    }

    /// 锁稳定读改写：在持有与 [`Inventory::set_stack`] 等同的写锁
    /// 期间，把槽位物品堆以可变借用交给回调，消除「`get_stack` 克隆
    /// → 改写 → `set_stack` 回写」两段锁之间的并发窗口与整栈克隆。
    ///
    /// 回调约定（违者丢更新或无法回滚）：
    /// - 返回 `true` 表示已修改物品堆，实现按各自 `set_stack` 的
    ///   副作用提交（含 `mark_dirty` 等）；返回 `false` 时**不得**
    ///   改动物品堆——覆写实现为原位修改，无法回滚已发生的改动；
    /// - 回调内不得调用同一物品栏的任何方法（覆写方在持锁状态下
    ///   回调，重入可能死锁），只能读写传入的物品堆。
    ///
    /// 返回回调是否提交了修改（未覆写回退路径下即是否有写入）。
    ///
    /// 默认实现回退为 `get_stack` + 条件 `set_stack`（语义等价，
    /// 仅失去锁稳定性与省克隆收益）。
    fn with_stack_mut(&self, slot: usize, f: &mut dyn FnMut(&mut ItemStack) -> bool) -> bool {
        let mut stack = self.get_stack(slot);
        let changed = f(&mut stack);
        if changed {
            self.set_stack(slot, stack);
        }
        changed
    }

    fn get_stack(&self, slot: usize) -> ItemStack;

    fn remove_stack(&self, slot: usize) -> ItemStack;

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack;

    fn set_stack(&self, slot: usize, stack: ItemStack);

    fn on_open(&self) {}
    fn on_close(&self) {}

    fn count(&self, item: &Item) -> u8 {
        let mut count = 0u8;

        for i in 0..self.size() {
            // 借用读取：逐槽深克隆组件补丁只为取 id 与数量，白付
            self.with_stack(i, &mut |stack| {
                if stack.get_item().id == item.id {
                    // saturating_add：全背包同物品总数超过 255 时 u8 裸加
                    // 会溢出（debug panic/release 回绕）
                    count = count.saturating_add(stack.item_count);
                }
            });
        }

        count
    }

    fn contains_any_predicate(&self, predicate: &(dyn Fn(&ItemStack) -> bool + Sync)) -> bool {
        let mut found = false;
        for i in 0..self.size() {
            // 谓词为纯读比较：借用读取避免逐槽深克隆
            self.with_stack(i, &mut |stack| {
                if !found && predicate(stack) {
                    found = true;
                }
            });
            if found {
                return true;
            }
        }

        false
    }

    fn contains_any(&self, items: &[Item]) -> bool {
        self.contains_any_predicate(&|stack| !stack.is_empty() && items.contains(stack.get_item()))
    }

    fn write_inventory_nbt(&self, nbt: &mut NbtCompound, include_empty: bool) {
        let mut slots = Vec::new();
        let size = self.size();

        for i in 0..size {
            // write_item_stack 是 ItemStack 自身方法（不触物品栏），
            // 借用读取避免未落盘前的整槽深克隆
            self.with_stack(i, &mut |stack| {
                if !stack.is_empty() {
                    let mut item_compound = NbtCompound::new();
                    item_compound.put_byte("Slot", i as i8);
                    stack.write_item_stack(&mut item_compound);
                    slots.push(NbtTag::Compound(item_compound));
                }
            });
        }

        if include_empty || !slots.is_empty() {
            nbt.put("Items", NbtTag::List(slots));
        }
    }

    fn get_max_count_per_stack(&self) -> u8 {
        99
    }

    fn mark_dirty(&self) {}

    fn read_data(&self, nbt: &NbtCompound, stacks: &mut [ItemStack]) {
        sync_read_items_from_nbt(nbt, stacks);
    }

    fn is_valid_slot_for(&self, _slot: usize, _stack: &ItemStack) -> bool {
        true
    }

    fn can_transfer_to(
        &self,
        _hopper_inventory: &dyn Inventory,
        _slot: usize,
        _stack: &ItemStack,
    ) -> bool {
        true
    }

    fn as_any(&self) -> &dyn Any;
}

pub trait Clearable {
    fn clear(&self);
}

pub fn sync_read_items_from_nbt(nbt: &NbtCompound, stacks: &mut [ItemStack]) {
    if let Some(inventory_list) = nbt.get_list("Items") {
        for tag in inventory_list {
            if let Some(item_compound) = tag.extract_compound()
                && let Some(slot_byte) = item_compound.get_byte("Slot")
            {
                let slot = slot_byte as usize;
                if slot < stacks.len()
                    && let Some(item_stack) = ItemStack::read_item_stack(item_compound)
                {
                    stacks[slot] = item_stack;
                }
            }
        }
    }
}

pub fn sync_write_items_to_nbt(items: &[ItemStack], nbt: &mut NbtCompound) {
    let mut slots = Vec::new();
    for (i, stack) in items.iter().enumerate() {
        if !stack.is_empty() {
            let mut item_nbt = NbtCompound::new();
            item_nbt.put_byte("Slot", i as i8);
            stack.write_item_stack(&mut item_nbt);
            slots.push(NbtTag::Compound(item_nbt));
        }
    }
    if !slots.is_empty() {
        nbt.put_list("Items", slots);
    }
}

pub struct ComparableInventory(pub Arc<dyn Inventory>);

impl PartialEq for ComparableInventory {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ComparableInventory {}

impl Hash for ComparableInventory {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let ptr = Arc::as_ptr(&self.0);
        ptr.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::SimpleInventory;

    /// 全背包同物品总数超过 255 时计数必须封顶而不是溢出
    /// （u8 裸加在 debug 下 panic、release 下回绕）。
    #[test]
    fn count_saturates_instead_of_overflowing() {
        let inventory = SimpleInventory::new(9);
        for i in 0..9 {
            inventory.set_stack(i, ItemStack::new(64, &Item::STONE));
        }
        // 9×64=576，远超 u8 上限
        assert_eq!(inventory.count(&Item::STONE), 255);
    }

    /// `with_stack` 借用访问：回调收到的是槽位当前内容（与
    /// `get_stack` 一致），越界槽位回退 EMPTY（Option 型实现）。
    #[test]
    fn with_stack_borrows_current_content() {
        let inventory = SimpleInventory::new(2);
        inventory.set_stack(0, ItemStack::new(3, &Item::STONE));

        let mut seen_count = 0;
        inventory.with_stack(0, &mut |stack| seen_count = stack.item_count);
        assert_eq!(seen_count, 3);

        inventory.with_stack(1, &mut |stack| assert!(stack.is_empty()));
        inventory.with_stack(99, &mut |stack| assert!(stack.is_empty()));
    }

    /// 伪造/损坏存档可能带越界 Slot 字节：读取必须忽略越界条目
    /// 而不是越界写入（驴/骡驮箱等固定尺寸物品栏的读取防线）。
    #[test]
    fn read_items_from_nbt_ignores_out_of_range_slots() {
        let mut nbt = NbtCompound::new();
        let mut in_range = NbtCompound::new();
        in_range.put_byte("Slot", 1);
        ItemStack::new(5, &Item::STONE).write_item_stack(&mut in_range);
        let mut out_of_range = NbtCompound::new();
        out_of_range.put_byte("Slot", 99);
        ItemStack::new(7, &Item::DIRT).write_item_stack(&mut out_of_range);
        nbt.put(
            "Items",
            NbtTag::List(vec![
                NbtTag::Compound(in_range),
                NbtTag::Compound(out_of_range),
            ]),
        );

        let mut stacks = vec![ItemStack::EMPTY.clone(); 3];
        sync_read_items_from_nbt(&nbt, &mut stacks);

        assert!(stacks[0].is_empty());
        assert_eq!(stacks[1].item_count, 5);
        assert!(stacks[2].is_empty(), "越界 Slot 条目必须被忽略");
    }
}
