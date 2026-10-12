use crate::block::entities::BlockEntity;
use papokin_data::item_stack::ItemStack;
use papokin_inventory::{Clearable, Inventory};
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::position::BlockPos;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ShelfBlockEntity {
    pub position: BlockPos,
    pub items: papokin_inventory::LazyItemSlots<{ Self::INVENTORY_SIZE }>,
    pub align_items_to_bottom: AtomicBool,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
}

impl BlockEntity for ShelfBlockEntity {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_inventory_nbt(nbt, true);
        if self.align_items_to_bottom.load(Ordering::Relaxed) {
            nbt.put_bool("align_items_to_bottom", true);
        }
    }

    fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let align_items_to_bottom = nbt.get_bool("align_items_to_bottom").unwrap_or(false);
        let shelf = Self {
            position,
            items: papokin_inventory::LazyItemSlots::new(),
            align_items_to_bottom: AtomicBool::new(align_items_to_bottom),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        };

        // fill 仅在确有物品时物化槽数组（空架保持零驻留）。
        shelf.items.fill_from_nbt(nbt);

        shelf
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn is_comparator_dirty(&self) -> bool {
        self.comparator_dirty.load(Ordering::Relaxed)
    }

    fn clear_comparator_dirty(&self) {
        self.comparator_dirty.store(false, Ordering::Relaxed);
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        self.items.try_write_to_nbt(&mut nbt);
        if self.align_items_to_bottom.load(Ordering::Relaxed) {
            nbt.put_bool("align_items_to_bottom", true);
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ShelfBlockEntity {
    pub const INVENTORY_SIZE: usize = 3;
    pub const ID: &'static str = "minecraft:shelf";

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            items: papokin_inventory::LazyItemSlots::new(),
            align_items_to_bottom: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }
}

impl Inventory for ShelfBlockEntity {
    fn size(&self) -> usize {
        Self::INVENTORY_SIZE
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn with_stack(&self, slot: usize, f: &mut dyn FnMut(&ItemStack)) {
        self.items.with_stack(slot, f);
    }
    fn with_stack_mut(&self, slot: usize, f: &mut dyn FnMut(&mut ItemStack) -> bool) -> bool {
        let changed = self.items.with_stack_mut(slot, f);
        if changed {
            self.mark_dirty();
        }
        changed
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        self.items.get_stack(slot)
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        let removed = self
            .items
            .with_write(|items| std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone()));
        self.mark_dirty();
        removed
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        let res = self.items.with_write(|items| {
            if !items[slot].is_empty() && amount > 0 {
                items[slot].split(amount)
            } else {
                ItemStack::EMPTY.clone()
            }
        });
        self.mark_dirty();
        res
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        self.items.with_write(|items| items[slot] = stack);
        self.mark_dirty();
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for ShelfBlockEntity {
    fn clear(&self) {
        self.items.clear();
        self.mark_dirty();
    }
}
