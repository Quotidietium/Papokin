use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use papokin_data::item_stack::ItemStack;
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_util::math::position::BlockPos;

use crate::block::entities::BlockEntity;
use crate::world::World;
use papokin_inventory::{Clearable, Inventory};

/// 对应原版的 `JukeboxBlockEntity`
pub struct JukeboxBlockEntity {
    position: BlockPos,
    /// 存储在唱片机中的唱片物品（NBT 中的 `RecordItem`）
    record_stack: Arc<Mutex<ItemStack>>,
    /// 自当前歌曲开始播放以来经过的刻数
    ticks_since_song_started: AtomicU64,
    /// 当前歌曲的时长（以刻为单位，未播放时为 0）
    song_length_ticks: AtomicU64,
    dirty: AtomicBool,
    comparator_dirty: AtomicBool,
}

const RECORD_ITEM_NBT_KEY: &str = "RecordItem";
const TICKS_SINCE_SONG_STARTED_NBT_KEY: &str = "ticks_since_song_started";

impl BlockEntity for JukeboxBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let record_stack = nbt
            .get_compound(RECORD_ITEM_NBT_KEY)
            .and_then(ItemStack::read_item_stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone());

        let ticks_since_song_started =
            nbt.get_long(TICKS_SINCE_SONG_STARTED_NBT_KEY).unwrap_or(0) as u64;

        Self {
            position,
            record_stack: Arc::new(Mutex::new(record_stack)),
            ticks_since_song_started: AtomicU64::new(ticks_since_song_started),
            song_length_ticks: AtomicU64::new(0), // 将在开始游玩时被设置
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let record = self
            .record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !record.is_empty() {
            let mut record_nbt = NbtCompound::new();
            record.write_item_stack(&mut record_nbt);
            nbt.put(RECORD_ITEM_NBT_KEY, record_nbt);
        }

        let ticks = self.ticks_since_song_started.load(Ordering::Relaxed);
        if ticks > 0 {
            nbt.put_long(TICKS_SINCE_SONG_STARTED_NBT_KEY, ticks as i64);
        }
    }

    fn tick(&self, _world: &Arc<World>) {
        // 如果正在播放，递增刻数
        let song_length = self.song_length_ticks.load(Ordering::Relaxed);
        if song_length > 0 {
            let ticks = self
                .ticks_since_song_started
                .fetch_add(1, Ordering::Relaxed);
            // 检查歌曲是否已结束
            if ticks >= song_length {
                self.stop_playing();
                // TODO: 将方块状态更新为 has_record = false？还是仅停止红石？
                // 在原版中，唱片保留但音乐停止，红石关闭
            }
        }
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

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(record) = self.record_stack.try_lock()
            && !record.is_empty()
        {
            let mut record_nbt = NbtCompound::new();
            record.write_item_stack(&mut record_nbt);
            nbt.put("RecordItem", NbtTag::Compound(record_nbt));
        }
        nbt.put_long(
            "ticks_since_song_started",
            self.ticks_since_song_started.load(Ordering::Relaxed) as i64,
        );
        Some(nbt)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }
}

impl JukeboxBlockEntity {
    pub const ID: &'static str = "minecraft:jukebox";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            record_stack: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            ticks_since_song_started: AtomicU64::new(0),
            song_length_ticks: AtomicU64::new(0),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    /// 获取当前的唱片物品堆
    pub fn get_record(&self) -> ItemStack {
        self.record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 设置唱片物品堆 - 与原版的 `setStack()` 一致
    /// 注意：调用方负责更新方块状态并播放音乐
    pub fn set_record(&self, stack: ItemStack) {
        *self
            .record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack;
        self.mark_dirty();
    }

    /// 清空物品堆并返回其中的物品——用于丢弃
    pub fn clear_record(&self) -> ItemStack {
        self.stop_playing();
        let mut record = self
            .record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let taken = record.clone();
        *record = ItemStack::EMPTY.clone();
        self.mark_dirty();
        taken
    }

    /// 以给定的刻数长度开始播放歌曲
    pub fn start_playing(&self, length_in_ticks: u64) {
        self.ticks_since_song_started.store(0, Ordering::Relaxed);
        self.song_length_ticks
            .store(length_in_ticks, Ordering::Relaxed);
        self.mark_dirty();
    }

    /// 停止播放当前歌曲
    pub fn stop_playing(&self) {
        self.ticks_since_song_started.store(0, Ordering::Relaxed);
        self.song_length_ticks.store(0, Ordering::Relaxed);
        self.mark_dirty();
    }

    /// 检查当前是否正在播放歌曲
    pub fn is_playing(&self) -> bool {
        let song_length = self.song_length_ticks.load(Ordering::Relaxed);
        if song_length == 0 {
            return false;
        }
        let ticks = self.ticks_since_song_started.load(Ordering::Relaxed);
        ticks < song_length
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }
}

/// 为唱片机实现单槽物品栏（对应原版的 `SingleStackInventory`）
impl Inventory for JukeboxBlockEntity {
    fn size(&self) -> usize {
        1
    }

    fn is_empty(&self) -> bool {
        self.record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    fn get_stack(&self, _slot: usize) -> ItemStack {
        self.record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn remove_stack(&self, _slot: usize) -> ItemStack {
        self.stop_playing();
        let mut record = self
            .record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let taken = record.clone();
        *record = ItemStack::EMPTY.clone();
        self.mark_dirty();
        taken
    }

    fn remove_stack_specific(&self, _slot: usize, _amount: u8) -> ItemStack {
        // 唱片机只能容纳一个物品，因此移除整个堆叠
        self.remove_stack(0)
    }

    fn set_stack(&self, _slot: usize, stack: ItemStack) {
        *self
            .record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack;
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

impl Clearable for JukeboxBlockEntity {
    fn clear(&self) {
        self.stop_playing();
        *self
            .record_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ItemStack::EMPTY.clone();
        self.mark_dirty();
    }
}
