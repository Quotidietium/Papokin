//! 物品栏同步处理器。
//!
//! 本模块处理服务器与客户端之间的物品栏状态同步
//! 以及已连接的客户端。它确保玩家在槽位中看到正确的物品，
//! 光标物品以及容器属性（如熔炉进度）。
//!
//! # Synchronization
//!
//! 同步处理器管理：
//! - 容器内容整体更新（在打开容器或发生重大变化时发送）
//! - 单个槽位更新（单个槽位发生变化时发送）
//! - 光标物品更新（鼠标光标正拿着的物品）
//! - 属性更新（容器专属数据，如熔炉燃烧时间）
//!
//! # Revision Tracking
//!
//! 每条同步消息都包含一个修订号，以确保客户端
//! 和服务器保持同步。如果客户端检测到不同步，可以请求一次完整的
//! 重新同步。

use std::sync::{Arc, Mutex};

use papokin_data::item_stack::ItemStack;
use papokin_protocol::{
    codec::{
        item_stack_seralizer::{ItemStackSerializer, OptionalItemStackHash},
        var_int::VarInt,
    },
    java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
    },
};

use crate::screen_handler::{InventoryPlayer, ScreenHandlerBehaviour};

/// 处理向特定玩家同步物品栏的操作。
///
/// 同步处理器保存玩家的引用并发送物品栏
/// 在容器状态变化时发送更新数据包。它负责：
/// - 完整内容同步
/// - 增量槽位更新
/// - 光标物品跟踪
/// - 属性（UI 元素）更新
pub struct SyncHandler {
    /// 需要同步物品栏更新的玩家。
    ///
    /// 在调用 `store_player` 附加玩家之前为 None。
    player: Mutex<Option<Arc<dyn InventoryPlayer>>>,
}

impl Default for SyncHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncHandler {
    /// 创建一个未绑定玩家的同步处理器。
    #[must_use]
    pub fn new() -> Self {
        Self {
            player: Mutex::new(None),
        }
    }

    /// 存储要同步的玩家。
    ///
    /// 必须在任何同步操作之前调用。
    pub fn store_player(&self, player: Arc<dyn InventoryPlayer>) {
        self.player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(player);
    }

    /// 发送完整的容器内容更新。
    ///
    /// 此方法将所有槽位、光标物品和属性发送给客户端。
    /// 用于初始同步以及从失步状态恢复。
    ///
    /// # Arguments
    /// - `screen_handler` - 要同步的屏幕处理器
    /// - `stacks` - 所有槽位的内容
    /// - `cursor_stack` - 光标所持的物品
    /// - `properties` - 容器的属性值
    /// - `next_revision` - 新的修订号
    pub fn update_state(
        &self,
        screen_handler: &ScreenHandlerBehaviour,
        stacks: &[ItemStack],
        cursor_stack: &ItemStack,
        properties: &[i32],
        next_revision: u32,
    ) {
        if let Some(player) = self
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            player.enqueue_inventory_packet(
                &CSetContainerContent::new(
                    VarInt(screen_handler.sync_id.into()),
                    VarInt(next_revision as i32),
                    stacks
                        .iter()
                        .map(|stack| ItemStackSerializer::from(stack.clone()))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    &ItemStackSerializer::from(cursor_stack.clone()),
                ),
                screen_handler.window_type,
            );

            for (i, property) in properties.iter().enumerate() {
                player.enqueue_property_packet(&CSetContainerProperty::new(
                    VarInt(screen_handler.sync_id.into()),
                    i as i16,
                    *property as i16,
                ));
            }
        }
    }

    /// 在客户端上更新单个槽位。
    ///
    /// 对于单个槽位的更改，比完全同步更高效。
    ///
    /// # Arguments
    /// - `screen_handler` - 屏幕处理器
    /// - `slot` - 发生变化的槽位索引
    /// - `stack` - 该槽位中的新物品堆
    /// - `next_revision` - 新的修订号
    pub fn update_slot(
        &self,
        screen_handler: &ScreenHandlerBehaviour,
        slot: usize,
        stack: &ItemStack,
        next_revision: u32,
    ) {
        if let Some(player) = self
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            player.enqueue_slot_packet(
                &CSetContainerSlot::new(
                    screen_handler.sync_id as i8,
                    next_revision as i32,
                    slot as i16,
                    &ItemStackSerializer::from(stack.clone()),
                ),
                screen_handler.window_type,
                screen_handler.slots.len(),
            );
        }
    }

    /// 在客户端上更新光标物品。
    ///
    /// 当玩家手持的（光标）物品发生变化时发送。
    ///
    /// # Arguments
    /// - `stack` - 光标处的新物品
    pub fn update_cursor_stack(&self, stack: &ItemStack) {
        if let Some(player) = self
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            player.enqueue_cursor_packet(&CSetCursorItem::new(&ItemStackSerializer::from(
                stack.clone(),
            )));
        }
    }

    /// 在客户端上更新一个容器属性。
    ///
    /// 用于熔炉进度条等 UI 元素。
    ///
    /// # Arguments
    /// - `screen_handler` - 屏幕处理器
    /// - `property` - 属性索引
    /// - `value` - 新的属性值
    pub fn update_property(
        &self,
        screen_handler: &ScreenHandlerBehaviour,
        property: i32,
        value: i32,
    ) {
        if let Some(player) = self
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            player.enqueue_property_packet(&CSetContainerProperty::new(
                VarInt(screen_handler.sync_id.into()),
                property as i16,
                value as i16,
            ));
        }
    }
}

/// 为同步目的跟踪槽位的最近已知状态。
///
/// 用于检测槽位何时发生变化并需要同步到客户端。
/// 存储完整物品堆或其哈希值，用于比较。
#[derive(Clone)]
pub struct TrackedStack {
    /// 最近一次发送给客户端的完整物品堆栈。
    ///
    /// 发送完整物品堆数据时设置。仅发送哈希时清除。
    pub received_stack: Option<ItemStack>,
    /// 最近一次发送给客户端的物品堆栈的哈希值。
    ///
    /// 用于轻量级比较以检测变化。
    pub received_hash: Option<OptionalItemStackHash>,
}

impl TrackedStack {
    /// 一个没有已知状态的空跟踪物品堆。
    pub const EMPTY: Self = Self {
        received_stack: None,
        received_hash: None,
    };

    /// 记录我们已将此物品堆发送给客户端。
    pub fn set_received_stack(&mut self, stack: ItemStack) {
        self.received_stack = Some(stack);
        self.received_hash = None;
    }

    /// 记录我们已将此哈希发送给客户端。
    pub fn set_received_hash(&mut self, hash: OptionalItemStackHash) {
        self.received_hash = Some(hash);
        self.received_stack = None;
    }

    /// 检查实际物品堆是否与所追踪的状态一致。
    ///
    /// 如果两者匹配，则将跟踪状态更新为实际物品堆。
    //FIX 名为 `is_*` 的方法通常通过引用或不带 self 的方式获取 self。考虑选择一个歧义更小的名称。
    pub fn is_in_sync(&mut self, actual_stack: &ItemStack) -> bool {
        if let Some(stack) = &self.received_stack {
            return stack.are_equal(actual_stack);
        } else if let Some(hash) = &self.received_hash
            && hash.hash_equals(actual_stack)
        {
            self.received_stack = Some(actual_stack.clone());
            return true;
        }

        false
    }
}
