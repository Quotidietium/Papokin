use std::sync::{Arc, Mutex as StdMutex, RwLock, atomic::AtomicBool};

use papokin_data::item_stack::ItemStack;
use papokin_util::math::position::BlockPos;

use crate::{
    block::viewer::ViewerCountTracker, impl_block_entity_for_chest, impl_chest_helper_methods,
    impl_clearable_for_chest, impl_inventory_for_chest, impl_viewer_count_listener_for_chest,
};

pub struct TrappedChestBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,

    // 观察者
    viewers: ViewerCountTracker,

    /// 待处理的战利品表键。在生成时设置，首次打开时清除。
    pub loot_table: StdMutex<Option<String>>,
    /// 用于确定性战利品生成的种子。
    pub loot_table_seed: i64,
}

impl TrappedChestBlockEntity {
    pub const INVENTORY_SIZE: usize = 27;
    pub const LID_ANIMATION_EVENT_TYPE: u8 = 1;
    pub const ID: &'static str = "minecraft:trapped_chest";
    pub const EMITS_REDSTONE: bool = true;
}

// 应用宏来生成 trait 实现
impl_block_entity_for_chest!(TrappedChestBlockEntity);
impl_inventory_for_chest!(TrappedChestBlockEntity);
impl_clearable_for_chest!(TrappedChestBlockEntity);
impl_viewer_count_listener_for_chest!(TrappedChestBlockEntity);
impl_chest_helper_methods!(TrappedChestBlockEntity);
