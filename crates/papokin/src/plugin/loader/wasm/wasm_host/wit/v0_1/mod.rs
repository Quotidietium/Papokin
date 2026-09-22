use crate::plugin::{
    PluginMetadata,
    loader::wasm::wasm_host::{
        PluginInitError, PluginInstance, concurrent_store::LegacySyncReentry,
        state::PluginHostState,
    },
};
use papokin_host_bindings::PluginPre;
use wasmtime::component::{HasSelf, InstancePre, Linker};
use wasmtime::{Engine, Store};

pub mod advancement;
// wasmtime 的 `bindgen!` 要求每个 Host trait 方法都是 `async fn`，即便是那些
// 这里的实现恰好不需要 `.await` 任何东西——因此 `unused_async_trait_impl`
// 若不破坏生成的 trait 签名就无法避免。
#[allow(clippy::unused_async_trait_impl)]
pub mod block_entity;
#[allow(clippy::unused_async_trait_impl)]
pub mod boss_bar;
#[allow(clippy::unused_async_trait_impl)]
pub mod commands;
#[allow(clippy::unused_async_trait_impl)]
pub mod common;
#[allow(clippy::unused_async_trait_impl)]
pub mod config;
#[allow(clippy::unused_async_trait_impl)]
pub mod context;
#[allow(clippy::unused_async_trait_impl)]
pub mod cookie;
#[allow(clippy::unused_async_trait_impl)]
pub mod damage_type;
#[allow(clippy::unused_async_trait_impl)]
pub mod datapack;
#[allow(clippy::unused_async_trait_impl)]
pub mod display;
#[allow(clippy::unused_async_trait_impl)]
pub mod dragon;
#[allow(clippy::unused_async_trait_impl)]
pub mod enchantment;
#[allow(clippy::unused_async_trait_impl)]
pub mod entity;
pub mod events;
pub mod generated_packets;
#[allow(clippy::unused_async_trait_impl)]
pub mod gui;
#[allow(clippy::unused_async_trait_impl)]
pub mod i18n;
#[allow(clippy::unused_async_trait_impl)]
pub mod inventory;
pub mod ipc;
#[allow(clippy::unused_async_trait_impl)]
pub mod item_stack;
pub mod java_dialogs;
#[allow(clippy::unused_async_trait_impl)]
pub mod living_entity;
#[allow(clippy::unused_async_trait_impl)]
pub mod logging;
#[allow(clippy::unused_async_trait_impl)]
pub mod loot;
#[allow(clippy::unused_async_trait_impl)]
pub mod map;
#[allow(clippy::unused_async_trait_impl)]
pub mod merchant;
#[allow(clippy::unused_async_trait_impl)]
pub mod messaging;
#[allow(clippy::unused_async_trait_impl)]
pub mod mob;
pub mod permission;
#[allow(clippy::unused_async_trait_impl)]
pub mod player;
#[allow(clippy::unused_async_trait_impl)]
pub mod recipe;
#[allow(clippy::unused_async_trait_impl)]
pub mod registry;
pub mod scheduler;
#[allow(clippy::unused_async_trait_impl)]
pub mod scoreboard;
#[allow(clippy::unused_async_trait_impl)]
pub mod server;
#[allow(clippy::unused_async_trait_impl)]
pub mod services;
pub mod status_effect;
#[allow(clippy::unused_async_trait_impl)]
pub mod structure;
#[allow(clippy::unused_async_trait_impl)]
pub mod tag;
#[allow(clippy::unused_async_trait_impl)]
pub mod text;
#[allow(clippy::unused_async_trait_impl)]
pub mod uuid;
#[allow(clippy::unused_async_trait_impl)]
pub mod world;

pub use papokin_host_bindings::{Plugin, papokin};

impl papokin::plugin::java_packets::Host for PluginHostState {}
impl papokin::plugin::data_components::Host for PluginHostState {}
impl papokin::plugin::enchantments::Host for PluginHostState {}
impl papokin::plugin::biomes::Host for PluginHostState {}
impl papokin::plugin::attributes::Host for PluginHostState {}
impl papokin::plugin::advancement::Host for PluginHostState {}
impl papokin::plugin::damage_types::Host for PluginHostState {}
impl papokin::plugin::tag::Host for PluginHostState {}
impl papokin::plugin::registry::Host for PluginHostState {}
impl papokin::plugin::combat::Host for PluginHostState {}
impl papokin::plugin::screens::Host for PluginHostState {}
impl papokin::plugin::statistics::Host for PluginHostState {}
impl papokin::plugin::game_rules::Host for PluginHostState {}
impl papokin::plugin::game_events::Host for PluginHostState {}
impl papokin::plugin::potions::Host for PluginHostState {}
impl papokin::plugin::entity_statuses::Host for PluginHostState {}

pub fn add_to_linker(linker: &mut Linker<PluginHostState>) -> wasmtime::Result<()> {
    Plugin::add_to_linker::<_, HasSelf<_>>(linker, |state: &mut PluginHostState| state)?;
    Ok(())
}

pub fn prepare_plugin(
    instance_pre: &InstancePre<PluginHostState>,
) -> wasmtime::Result<PluginPre<PluginHostState>> {
    PluginPre::new(instance_pre.clone())
}

pub async fn init_plugin(
    engine: &Engine,
    plugin_pre: PluginPre<PluginHostState>,
    legacy_sync_reentry: &LegacySyncReentry,
) -> Result<(PluginInstance, Store<PluginHostState>, PluginMetadata), PluginInitError> {
    let mut store = Store::new(engine, PluginHostState::new());
    store.limiter(|state| &mut state.limits);
    let plugin = legacy_sync_reentry
        .scope_bootstrap(plugin_pre.instantiate_async(&mut store))
        .await
        .map_err(PluginInitError::InstantiationFailed)?;

    store
        .run_concurrent(async |accessor| {
            legacy_sync_reentry
                .scope_bootstrap(plugin.call_init_plugin(accessor))
                .await
        })
        .await
        .map_err(PluginInitError::CallInitPluginFailed)?
        .map_err(PluginInitError::CallInitPluginFailed)?;

    let metadata = store
        .run_concurrent(async |accessor| {
            legacy_sync_reentry
                .scope_bootstrap(plugin.papokin_plugin_metadata().call_get_metadata(accessor))
                .await
        })
        .await
        .map_err(PluginInitError::CallGetMetadataFailed)?
        .map_err(PluginInitError::CallGetMetadataFailed)?;

    let metadata = PluginMetadata {
        name: metadata.name,
        version: metadata.version,
        authors: metadata.authors,
        description: metadata.description,
        dependencies: metadata.dependencies,
        permissions: metadata.permissions,
        load_after: metadata.load_after,
        load_before: metadata.load_before,
        provides: metadata.provides,
        load_order: match metadata.load_order {
            papokin_host_bindings::exports::papokin::plugin::metadata::LoadOrder::Startup => {
                crate::plugin::LoadOrder::Startup
            }
            papokin_host_bindings::exports::papokin::plugin::metadata::LoadOrder::PostWorld => {
                crate::plugin::LoadOrder::PostWorld
            }
        },
    };

    store
        .data_mut()
        .permissions
        .clone_from(&metadata.permissions);

    Ok((PluginInstance::V0_1(plugin), store, metadata))
}
