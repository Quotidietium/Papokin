use papokin_data::data_component_impl::ContainerImpl;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::position::BlockPos;
use std::any::Any;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{array::from_fn, sync::Arc};

use crate::block::entities::BlockEntity;
use crate::block::viewer::{ViewerCountListener, ViewerCountTracker, ViewerCountTrackerExt};
use crate::world::World;
use papokin_inventory::{Clearable, Inventory, sync_write_items_to_nbt};

pub struct ShulkerBoxBlockEntity {
    pub position: BlockPos,
    pub items: papokin_inventory::LazyItemSlots<{ Self::INVENTORY_SIZE }>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,

    // 观察者
    pub viewers: ViewerCountTracker,
}

impl BlockEntity for ShulkerBoxBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let shulker_box = Self {
            position,
            items: papokin_inventory::LazyItemSlots::new(),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            viewers: ViewerCountTracker::new(),
        };

        // fill 仅在确有物品时物化槽数组（空箱保持零驻留）。
        shulker_box.items.fill_from_nbt(nbt);

        shulker_box
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_inventory_nbt(nbt, true);
    }

    fn tick(&self, world: &Arc<World>) {
        self.viewers
            .update_viewer_count::<Self>(self, world, &self.position);
    }

    fn on_block_replaced(self: Arc<Self>, _world: &Arc<World>, _position: &BlockPos) {
        // 潜影盒被破坏时保留物品
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn collect_item_components(&self, stack: &mut ItemStack) {
        let contents: Vec<(u8, ItemStack)> = self.items.with_read(|items| {
            items.map_or_else(Vec::new, |items| {
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| !item.is_empty())
                    .map(|(slot, item)| (slot as u8, item.clone()))
                    .collect()
            })
        });

        if !contents.is_empty() {
            stack.set_data_component(ContainerImpl { items: contents });
        }
    }

    fn apply_item_components(&self, stack: &ItemStack) {
        let Some(container) = stack.get_data_component::<ContainerImpl>() else {
            return;
        };

        if container.items.is_empty() {
            return;
        }

        self.items.with_write(|items| {
            for (slot, item) in &container.items {
                if let Some(target) = items.get_mut(*slot as usize) {
                    *target = item.clone();
                }
            }
        });

        self.mark_dirty();
    }

    fn drops_for_creative_player(&self) -> bool {
        !self.is_empty()
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
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ViewerCountListener for ShulkerBoxBlockEntity {
    fn on_container_open(&self, world: &Arc<World>, position: &BlockPos) {
        Self::play_sound(world, position, 1);
        // TODO: this.world.emitGameEvent(player, GameEvent.CONTAINER_OPEN, this.pos);
    }

    fn on_container_close(&self, world: &Arc<World>, position: &BlockPos) {
        Self::play_sound(world, position, 0);
        // TODO: this.world.emitGameEvent(player, GameEvent.CONTAINER_CLOSE, this.pos);
    }

    fn on_viewer_count_update(&self, world: &Arc<World>, position: &BlockPos, _old: u16, new: u16) {
        world.add_synced_block_event(*position, Self::OPEN_ANIMATION_EVENT_TYPE, new as u8);
    }
}

impl ShulkerBoxBlockEntity {
    pub const INVENTORY_SIZE: usize = 27;
    pub const OPEN_ANIMATION_EVENT_TYPE: u8 = 1;
    pub const ID: &'static str = "minecraft:shulker_box";

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            items: papokin_inventory::LazyItemSlots::new(),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            viewers: ViewerCountTracker::new(),
        }
    }

    pub fn update_viewers(&self, world: &Arc<World>) {
        let viewer_count = self.viewers.current.load(Ordering::Relaxed);
        Self::play_sound(world, &self.position, i32::from(viewer_count));
    }

    fn play_sound(world: &World, position: &BlockPos, viewer_count: i32) {
        let sound = if viewer_count > 0 {
            Sound::BlockShulkerBoxOpen
        } else {
            Sound::BlockShulkerBoxClose
        };

        world.play_sound(sound, SoundCategory::Blocks, &position.to_f64());
    }
}

impl Inventory for ShulkerBoxBlockEntity {
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

    fn on_open(&self) {
        self.viewers.open_container();
    }

    fn on_close(&self) {
        self.viewers.close_container();
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for ShulkerBoxBlockEntity {
    fn clear(&self) {
        self.items.clear();
        self.mark_dirty();
    }
}
