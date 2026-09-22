use std::sync::{Arc, Mutex as StdMutex, RwLock, atomic::AtomicBool};

use papokin_data::item_stack::ItemStack;
use papokin_util::math::position::BlockPos;

use crate::{
    block::viewer::ViewerCountTracker, impl_block_entity_for_chest, impl_chest_helper_methods,
    impl_clearable_for_chest, impl_inventory_for_chest, impl_viewer_count_listener_for_chest,
};

pub struct ChestBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,

    // 观察者
    viewers: ViewerCountTracker,

    /// 待处理的战利品表键（例如 `"minecraft:chests/simple_dungeon"`）。
    /// 在世界生成时设置；首次打开生成物品时清除。
    pub loot_table: StdMutex<Option<String>>,
    /// 用于确定性战利品生成的种子，与 `loot_table` 配对。
    pub loot_table_seed: i64,
}

impl ChestBlockEntity {
    pub const INVENTORY_SIZE: usize = 27;
    pub const LID_ANIMATION_EVENT_TYPE: u8 = 1;
    pub const ID: &'static str = "minecraft:chest";
    pub const EMITS_REDSTONE: bool = false;
}

// 应用宏来生成 trait 实现
impl_block_entity_for_chest!(ChestBlockEntity);
impl_inventory_for_chest!(ChestBlockEntity);
impl_clearable_for_chest!(ChestBlockEntity);
impl_viewer_count_listener_for_chest!(ChestBlockEntity);
impl_chest_helper_methods!(ChestBlockEntity);
