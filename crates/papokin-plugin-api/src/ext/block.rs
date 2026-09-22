use crate::wit::papokin::plugin::world::{
    Block, BlockState, get_all_block_names, get_all_blocks, get_block_by_id, get_block_by_name,
    get_block_count, get_block_from_state, get_block_from_state_id, get_block_properties,
    get_block_state_by_id, get_block_state_count, get_default_state_from_block,
    get_default_state_from_block_id, get_state_ids_for_block_id, get_states_for_block,
    get_states_for_block_id,
};

impl Block {
    ///返回注册表中所有已注册的方块。
    #[must_use]
    pub fn all() -> Vec<Self> {
        get_all_blocks()
    }

    /// 返回所有已注册方块的名称。
    #[must_use]
    pub fn all_names() -> Vec<String> {
        get_all_block_names()
    }

    /// 返回已注册方块类型的总数。
    #[must_use]
    pub fn count() -> u32 {
        get_block_count()
    }

    /// 返回已注册方块状态的总数。
    #[must_use]
    pub fn total_state_count() -> u32 {
        get_block_state_count()
    }

    /// 按数字方块 ID 获取方块定义。
    #[must_use]
    pub fn from_id(id: u16) -> Option<Self> {
        get_block_by_id(id)
    }

    /// 按命名空间名称（如 "minecraft:stone" 或 "stone"）获取方块定义。
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        get_block_by_name(name)
    }

    /// 获取给定方块状态 ID 对应的方块定义。
    #[must_use]
    pub fn from_state_id(state_id: u16) -> Option<Self> {
        get_block_from_state_id(state_id)
    }

    /// 获取给定方块状态对应的方块定义。
    #[must_use]
    pub fn from_state(state: &BlockState) -> Self {
        get_block_from_state(state)
    }

    /// 获取该方块类型的所有合法方块状态。
    #[must_use]
    pub fn get_states(&self) -> Vec<BlockState> {
        get_states_for_block(self)
    }

    /// 获取给定数字方块 ID 的所有合法方块状态。
    #[must_use]
    pub fn get_states_for_id(block_id: u16) -> Vec<BlockState> {
        get_states_for_block_id(block_id)
    }

    /// 获取给定数字方块 ID 的所有合法方块状态 ID。
    #[must_use]
    pub fn get_state_ids_for_id(block_id: u16) -> Vec<u16> {
        get_state_ids_for_block_id(block_id)
    }

    /// 获取此方块的默认方块状态。
    #[must_use]
    pub fn get_default_state(&self) -> BlockState {
        get_default_state_from_block(self)
    }

    /// 获取给定数字方块 ID 的默认方块状态。
    #[must_use]
    pub fn get_default_state_for_id(block_id: u16) -> Option<BlockState> {
        get_default_state_from_block_id(block_id)
    }
}

impl BlockState {
    /// 获取该方块状态所属的方块定义。
    #[must_use]
    pub fn get_block(&self) -> Block {
        get_block_from_state(self)
    }

    /// 按数字方块状态 ID 获取详细方块状态。
    #[must_use]
    pub fn from_id(state_id: u16) -> Option<Self> {
        get_block_state_by_id(state_id)
    }

    /// 获取该方块状态的属性键值对。
    #[must_use]
    pub fn get_properties(&self) -> Vec<(String, String)> {
        get_block_properties(self.id)
    }
}
