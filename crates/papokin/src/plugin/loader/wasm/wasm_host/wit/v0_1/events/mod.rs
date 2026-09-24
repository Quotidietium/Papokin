use std::sync::Arc;

use papokin_data::{Block, entity::EntityType};
use papokin_inventory::screen_handler::ClickType;
use papokin_protocol::java::server::play::ActionType;
use papokin_util::{
    GameMode, Hand,
    math::{position::BlockPos, vector3::Vector3},
};
use wasmtime::component::Resource;

use crate::{
    entity::player::Player,
    plugin::{
        BoxFuture, EventHandler, Payload,
        loader::wasm::wasm_host::{
            PluginInstance, WasmPlugin,
            state::{PlayerResource, PluginHostState, TextComponentResource, WorldResource},
            wit::{self, v0_1::papokin},
        },
    },
    server::Server,
    world::World,
};

pub mod block;
pub mod cleanup;
pub mod dialog;
pub mod enchantment;
pub mod entity;
pub mod hanging;
pub mod inventory;
pub mod player;
pub mod raid;
pub mod server;
pub mod vehicle;
pub mod world;

pub use cleanup::*;

impl papokin::plugin::event::Host for PluginHostState {}

pub struct WasmPluginEventHandler {
    pub handler_id: u32,
    pub plugin: Arc<WasmPlugin>,
    /// Bukkit 风格的 `ignoreCancelled`：当……时调度器跳过此处理器
    /// 触发的事件报告 `cancelled_state() == Some(true)`。
    pub ignore_cancelled: bool,
}

pub trait ToFromWasmEvent {
    fn to_wasm_event(
        &self,
        state: &mut PluginHostState,
    ) -> wit::v0_1::papokin::plugin::event::Event;

    fn from_wasm_event(
        event: wit::v0_1::papokin::plugin::event::Event,
        state: &mut PluginHostState,
    ) -> Self;

    fn apply_wasm_event(
        &mut self,
        event: wit::v0_1::papokin::plugin::event::Event,
        state: &mut PluginHostState,
    ) where
        Self: Sized,
    {
        *self = Self::from_wasm_event(event, state);
    }
}

pub(super) const fn to_wasm_position(position: Vector3<f64>) -> papokin::plugin::common::Position {
    (position.x, position.y, position.z)
}

pub(super) const fn from_wasm_position(
    position: papokin::plugin::common::Position,
) -> Vector3<f64> {
    Vector3::new(position.0, position.1, position.2)
}

pub(super) const fn to_wasm_block_position(
    position: BlockPos,
) -> papokin::plugin::common::BlockPos {
    papokin::plugin::common::BlockPos {
        x: position.0.x,
        y: position.0.y,
        z: position.0.z,
    }
}

pub(super) const fn from_wasm_block_position(
    position: papokin::plugin::common::BlockPos,
) -> BlockPos {
    BlockPos::new(position.x, position.y, position.z)
}

pub(super) fn to_wasm_block_name(block: &'static Block) -> String {
    format!("minecraft:{}", block.name)
}

pub(super) fn from_wasm_block_name(block_name: &str) -> &'static Block {
    Block::from_registry_key(block_name.strip_prefix("minecraft:").unwrap_or(block_name))
        .unwrap_or(&Block::AIR)
}

pub(super) fn to_wasm_entity_type(entity_type: &'static EntityType) -> String {
    format!("minecraft:{}", entity_type.resource_name)
}

pub(super) fn from_wasm_entity_type(entity_type: &str) -> &'static EntityType {
    EntityType::from_name(
        entity_type
            .strip_prefix("minecraft:")
            .unwrap_or(entity_type),
    )
    .unwrap_or(&EntityType::PLAYER)
}

pub(super) const fn to_wasm_hand(hand: Hand) -> papokin::plugin::common::Hand {
    match hand {
        Hand::Left => papokin::plugin::common::Hand::Left,
        Hand::Right => papokin::plugin::common::Hand::Right,
    }
}

pub(super) const fn from_wasm_hand(hand: papokin::plugin::common::Hand) -> Hand {
    match hand {
        papokin::plugin::common::Hand::Left => Hand::Left,
        papokin::plugin::common::Hand::Right => Hand::Right,
    }
}

pub(super) const fn to_wasm_entity_interaction_action(
    action: ActionType,
) -> papokin::plugin::event::EntityInteractionAction {
    match action {
        ActionType::Interact => papokin::plugin::event::EntityInteractionAction::Interact,
        ActionType::Attack => papokin::plugin::event::EntityInteractionAction::Attack,
        ActionType::InteractAt => papokin::plugin::event::EntityInteractionAction::InteractAt,
    }
}

pub(super) const fn from_wasm_entity_interaction_action(
    action: papokin::plugin::event::EntityInteractionAction,
) -> ActionType {
    match action {
        papokin::plugin::event::EntityInteractionAction::Interact => ActionType::Interact,
        papokin::plugin::event::EntityInteractionAction::Attack => ActionType::Attack,
        papokin::plugin::event::EntityInteractionAction::InteractAt => ActionType::InteractAt,
    }
}

pub(super) const fn to_wasm_game_mode(game_mode: GameMode) -> papokin::plugin::common::GameMode {
    match game_mode {
        GameMode::Survival => papokin::plugin::common::GameMode::Survival,
        GameMode::Creative => papokin::plugin::common::GameMode::Creative,
        GameMode::Adventure => papokin::plugin::common::GameMode::Adventure,
        GameMode::Spectator => papokin::plugin::common::GameMode::Spectator,
    }
}

pub(super) const fn from_wasm_game_mode(game_mode: papokin::plugin::common::GameMode) -> GameMode {
    match game_mode {
        papokin::plugin::common::GameMode::Survival => GameMode::Survival,
        papokin::plugin::common::GameMode::Creative => GameMode::Creative,
        papokin::plugin::common::GameMode::Adventure => GameMode::Adventure,
        papokin::plugin::common::GameMode::Spectator => GameMode::Spectator,
    }
}

pub(super) const fn to_wasm_click_type(click_type: ClickType) -> papokin::plugin::gui::ClickType {
    match click_type {
        ClickType::Left => papokin::plugin::gui::ClickType::Left,
        ClickType::Right => papokin::plugin::gui::ClickType::Right,
        ClickType::ShiftLeft => papokin::plugin::gui::ClickType::ShiftLeft,
        ClickType::ShiftRight => papokin::plugin::gui::ClickType::ShiftRight,
        ClickType::Middle => papokin::plugin::gui::ClickType::Middle,
        ClickType::Drop => papokin::plugin::gui::ClickType::Drop,
        ClickType::ControlDrop => papokin::plugin::gui::ClickType::ControlDrop,
        ClickType::DoubleClick => papokin::plugin::gui::ClickType::DoubleClick,
        ClickType::NumberKey(_) => papokin::plugin::gui::ClickType::NumberKey,
        ClickType::Unknown => papokin::plugin::gui::ClickType::Unknown,
    }
}

pub(super) const fn from_wasm_click_type(click_type: papokin::plugin::gui::ClickType) -> ClickType {
    match click_type {
        papokin::plugin::gui::ClickType::Left => ClickType::Left,
        papokin::plugin::gui::ClickType::Right => ClickType::Right,
        papokin::plugin::gui::ClickType::ShiftLeft => ClickType::ShiftLeft,
        papokin::plugin::gui::ClickType::ShiftRight => ClickType::ShiftRight,
        papokin::plugin::gui::ClickType::Middle => ClickType::Middle,
        papokin::plugin::gui::ClickType::Drop => ClickType::Drop,
        papokin::plugin::gui::ClickType::ControlDrop => ClickType::ControlDrop,
        papokin::plugin::gui::ClickType::DoubleClick => ClickType::DoubleClick,
        papokin::plugin::gui::ClickType::NumberKey => ClickType::NumberKey(0), // 默认为 0
        papokin::plugin::gui::ClickType::Unknown => ClickType::Unknown,
    }
}

pub(super) fn consume_player(
    state: &mut PluginHostState,
    player: &Resource<papokin::plugin::player::Player>,
) -> Arc<Player> {
    state
        .resource_table
        .delete::<PlayerResource>(Resource::new_own(player.rep()))
        .expect("无效的玩家资源句柄")
        .provider
}

pub(super) fn consume_text_component(
    state: &mut PluginHostState,
    text_component: &Resource<papokin::plugin::text::TextComponent>,
) -> papokin_util::text::TextComponent {
    state
        .resource_table
        .delete::<TextComponentResource>(Resource::new_own(text_component.rep()))
        .expect("无效的文本组件资源句柄")
        .provider
}

pub(super) fn consume_world(
    state: &mut PluginHostState,
    world: &Resource<papokin::plugin::world::World>,
) -> Arc<World> {
    state
        .resource_table
        .delete::<WorldResource>(Resource::new_own(world.rep()))
        .expect("无效的世界资源句柄")
        .provider
}

impl<E: Payload + ToFromWasmEvent + Clone + 'static> EventHandler<E> for WasmPluginEventHandler {
    fn handle<'a>(&'a self, server: &'a Arc<Server>, event: &'a E) -> BoxFuture<'a, ()> {
        Box::pin(async {
            let event = event.clone();
            let server = server.clone();
            let handler_id = self.handler_id;
            let function = match self.plugin.plugin_instance.as_ref() {
                PluginInstance::V0_1(plugin) => plugin.func_handle_event(),
            };
            if let Err(error) = self
                .plugin
                .store
                .call_guest(move |mut guest| {
                    Box::pin(async move {
                        let (wasm_event, server_res) = match guest.with(|mut store| {
                            store.data_mut().begin_dispatch_guard();
                            let wasm_event = event.to_wasm_event(store.data_mut());
                            match store.data_mut().add_server(server) {
                                Ok(resource) => Ok((wasm_event, resource)),
                                Err(error) => {
                                    // 降载服务器资源失败：按事件结构回收并重放
                                    // 护栏兜底（重复删除为无害 no-op）。
                                    cleanup_event(&wasm_event, store.data_mut());
                                    store.data_mut().end_dispatch_guard_failure();
                                    Err(error)
                                }
                            }
                        }) {
                            Ok(prepared) => prepared,
                            Err(error) => return Err(error),
                        };
                        // 降载会将这些资源转移给访客。仅当
                        // 成功返回的事件重新由宿主所有。
                        let result = guest
                            .call(function, (handler_id, server_res, wasm_event))
                            .await
                            .map(|(returned_event,)| returned_event);
                        match &result {
                            Ok(returned_event) => {
                                guest.with(|mut store| {
                                    cleanup_event(returned_event, store.data_mut());
                                    store.data_mut().end_dispatch_guard_success();
                                });
                            }
                            Err(_) => {
                                // 访客 trap/panic：参数资源无人释放，重放
                                // 护栏回收，防止失败派发把资源表刷满。
                                guest.with(|mut store| {
                                    store.data_mut().end_dispatch_guard_failure();
                                });
                            }
                        }
                        result.map(|_| ())
                    })
                })
                .await
            {
                tracing::error!(handler_id, %error, "Wasm 事件处理器执行失败");
            }
        })
    }

    fn handle_blocking<'a>(
        &'a self,
        server: &'a Arc<Server>,
        event: &'a mut E,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {
            let owned_event = event.clone();
            let server = server.clone();
            let handler_id = self.handler_id;
            let function = match self.plugin.plugin_instance.as_ref() {
                PluginInstance::V0_1(plugin) => plugin.func_handle_event(),
            };
            let result = self
                .plugin
                .store
                .call_guest(move |mut guest| {
                    Box::pin(async move {
                        let (wasm_event, server_res) = match guest.with(|mut store| {
                            store.data_mut().begin_dispatch_guard();
                            let wasm_event = owned_event.to_wasm_event(store.data_mut());
                            match store.data_mut().add_server(server) {
                                Ok(resource) => Ok((wasm_event, resource)),
                                Err(error) => {
                                    cleanup_event(&wasm_event, store.data_mut());
                                    store.data_mut().end_dispatch_guard_failure();
                                    Err(error)
                                }
                            }
                        }) {
                            Ok(prepared) => prepared,
                            Err(error) => return Err(error),
                        };
                        // 降载会将这些资源转移给访客。仅当
                        // 成功返回的事件重新由宿主所有。
                        let result = guest
                            .call(function, (handler_id, server_res, wasm_event))
                            .await
                            .map(|(returned_event,)| returned_event);
                        match result {
                            Ok(returned_event) => Ok(guest.with(|mut store| {
                                let mut updated_event = owned_event;
                                // 访客完全控制返回的事件变体；类型不匹配时
                                // from_wasm_event 会 panic（约半数事件走无守卫的
                                // 默认 apply_wasm_event）。绝不能让该 panic 跨越
                                // 宿主调用边界——拦截后忽略插件对本事件的全部修改。
                                let applied =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        updated_event
                                            .apply_wasm_event(returned_event, store.data_mut());
                                    }));
                                if applied.is_ok() {
                                    // from_wasm_event 的 consume_* 已按序消费全部
                                    // 资源；只清空记录、不重放。
                                    store.data_mut().end_dispatch_guard_success();
                                } else {
                                    tracing::error!(
                                        "插件事件处理器返回了类型不匹配的事件变体；已忽略其全部修改"
                                    );
                                    // 中途 panic 只消费了部分资源，重放护栏回收
                                    // 其余部分。
                                    store.data_mut().end_dispatch_guard_failure();
                                }
                                updated_event
                            })),
                            Err(error) => {
                                guest.with(|mut store| {
                                    store.data_mut().end_dispatch_guard_failure();
                                });
                                Err(error)
                            }
                        }
                    })
                })
                .await;
            match result {
                Ok(returned_event) => *event = returned_event,
                Err(error) => {
                    tracing::error!(handler_id, %error, "阻塞式 Wasm 事件处理器执行失败");
                }
            }
        })
    }
}
