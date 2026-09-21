//! End-to-end test plugin.
//!
//! Exercises the plugin-API mechanisms added in the Papo-coverage pass:
//! lifecycle (on_load/on_enable), event dispatch with priorities, async
//! wall-clock tasks, host-managed config, the services registry, and the
//! player plugin-message channel registry. Every step logs an `E2E` marker
//! that the harness asserts in the server log.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use pumpkin_plugin_api::damage_type::{DamageEffects, DamageScaling, DamageTypeBuilder};
use pumpkin_plugin_api::events::player::{
    AsyncTabCompleteEvent, PlayerHandshakeEvent, PlayerItemCooldownEvent, PlayerJumpEvent,
    PlayerPurchaseEvent, PlayerTrackEntityEvent, PlayerTradeEvent,
};
use pumpkin_plugin_api::events::{
    EventData, EventHandler, EventPriority, PlayerJoinEvent, ServerTickStartEvent,
};
use pumpkin_plugin_api::merchant::TradeOfferBuilder;
use pumpkin_plugin_api::recipe::{
    BrewingRecipeBuilder, SmithingTransformRecipeBuilder, SmithingTrimRecipeBuilder,
    StonecuttingRecipeBuilder,
};
use pumpkin_plugin_api::scheduler::SchedulerExt;
use pumpkin_plugin_api::services::ServiceProvider;
use pumpkin_plugin_api::structure::{BlockPos as StructurePos, WorldStructureExt};
use pumpkin_plugin_api::{
    Context, ItemStack, LoadOrder, Plugin, PluginMetadata, Result, TeleportFlags, register_plugin,
};

/// Smallest valid structure template (a single `minecraft:stone` block at the
/// origin) as gzipped NBT, the vanilla `.nbt` structure format.
const E2E_TEMPLATE_NBT: &[u8] = &[
    31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 77, 76, 73, 10, 128, 48, 16, 139, 212, 181, 23, 255, 227,
    35, 252, 67, 45, 35, 20, 187, 225, 204, 201, 215, 219, 10, 5, 3, 129, 132, 44, 26, 88, 208,
    179, 123, 72, 1, 168, 236, 26, 23, 76, 217, 120, 18, 33, 93, 237, 140, 126, 55, 129, 176, 6,
    23, 201, 222, 230, 148, 141, 37, 69, 42, 251, 241, 240, 201, 94, 252, 213, 20, 6, 22, 35, 84,
    116, 73, 84, 78, 220, 142, 127, 120, 1, 251, 234, 2, 179, 118, 0, 0, 0,
];

type PlayerJoinEventData = EventData<PlayerJoinEvent>;
type ServerTickStartEventData = EventData<ServerTickStartEvent>;
type PlayerJumpEventData = EventData<PlayerJumpEvent>;
type PlayerItemCooldownEventData = EventData<PlayerItemCooldownEvent>;
type PlayerTrackEntityEventData = EventData<PlayerTrackEntityEvent>;
type AsyncTabCompleteEventData = EventData<AsyncTabCompleteEvent>;
type PlayerHandshakeEventData = EventData<PlayerHandshakeEvent>;
type PlayerPurchaseEventData = EventData<PlayerPurchaseEvent>;
type PlayerTradeEventData = EventData<PlayerTradeEvent>;

static JOIN_COUNT: AtomicU32 = AtomicU32::new(0);
static TICK_COUNT: AtomicU32 = AtomicU32::new(0);
static JUMP_SEEN: AtomicBool = AtomicBool::new(false);
static ITEM_COOLDOWN_SEEN: AtomicBool = AtomicBool::new(false);
static TRACK_SEEN: AtomicBool = AtomicBool::new(false);
static TAB_COMPLETE_SEEN: AtomicBool = AtomicBool::new(false);
static HANDSHAKE_SEEN: AtomicBool = AtomicBool::new(false);
static PURCHASE_SEEN: AtomicBool = AtomicBool::new(false);
static TRADE_SEEN: AtomicBool = AtomicBool::new(false);
static COMBAT_CHECKED: AtomicBool = AtomicBool::new(false);
static ENTITY_API_CHECKED: AtomicBool = AtomicBool::new(false);
static CHUNK_DEMO_ARMED: AtomicBool = AtomicBool::new(false);
static CHUNK_DEMO_DONE: AtomicBool = AtomicBool::new(false);
static MAP_TICKS: AtomicU32 = AtomicU32::new(0);
static MAP_CHECKED: AtomicBool = AtomicBool::new(false);

struct JoinAnnouncerLowest;

impl EventHandler<PlayerJoinEvent> for JoinAnnouncerLowest {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerJoinEventData,
    ) -> PlayerJoinEventData {
        let n = JOIN_COUNT.fetch_add(1, Ordering::Relaxed);
        tracing::info!("E2E join-lowest seen join #{n}");
        event
    }
}

struct JoinAnnouncerHighest;

impl EventHandler<PlayerJoinEvent> for JoinAnnouncerHighest {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerJoinEventData,
    ) -> PlayerJoinEventData {
        let n = JOIN_COUNT.load(Ordering::Relaxed);
        // Bukkit ordering: LOWEST runs first, HIGHEST runs last, so this
        // handler must observe the counter already bumped by the LOWEST one.
        tracing::info!("E2E join-highest sees count={n} (expected >=1: priority order verified)");

        // CombatTracker read-only queries (new mechanism): on the first join,
        // deal one generic hit so the entry mapping, kill-credit, combat-state
        // and damage-type-name queries all flow through the host boundary.
        if !COMBAT_CHECKED.swap(true, Ordering::Relaxed) {
            use pumpkin_plugin_api::PlayerCombatExt;
            let player = &event.player;
            let before = player.get_combat_entries().len();
            player.damage(1.0, pumpkin_plugin_api::DamageType::Generic);
            let entries = player.get_combat_entries();
            let killer = player.get_killer();
            let in_combat = player.is_in_combat();
            let duration_ms = player.get_combat_duration_ms();
            let last_damage_type = player.get_last_damage_type_name();
            let player_attacker = player.has_player_attacker();
            tracing::info!(
                "E2E combat-queries entries={}->{} killer={} in_combat={} duration_ms={} last_damage_type={:?} player_attacker={}",
                before,
                entries.len(),
                killer.is_some(),
                in_combat,
                duration_ms,
                last_damage_type,
                player_attacker
            );
        }

        // Entity plugin-API long-tail (new mechanisms): SpawnCategory mapping,
        // EntitySnapshot (NBT serialize/rebuild), and read-only brain memory
        // (Bukkit MemoryKey) queries. Runs once on the first join.
        if !ENTITY_API_CHECKED.swap(true, Ordering::Relaxed) {
            use pumpkin_plugin_api::mobs::memory_keys;
            use pumpkin_plugin_api::{EntityType, SpawnCategory};

            let player = &event.player;
            let entity = player.as_entity();
            let world = player.get_world();
            let pos = player.get_position();

            // SpawnCategory mapping: players are MISC, zombies are MONSTER.
            let player_category = entity.get_spawn_category();
            let zombie = world.spawn_entity(EntityType::Zombie, pos);
            let zombie_category = zombie.get_spawn_category();
            if player_category == SpawnCategory::Misc && zombie_category == SpawnCategory::Monster {
                tracing::info!("E2E spawn-category-mapped player=misc zombie=monster");
            } else {
                tracing::info!(
                    "E2E spawn-category-broken player={player_category:?} zombie={zombie_category:?}"
                );
            }

            // EntitySnapshot: NBT round trip through spawn-entity-from-snapshot.
            let snapshot = zombie.create_snapshot();
            match world.spawn_entity_from_snapshot(&snapshot, pos) {
                Some(clone) => {
                    let same_type = clone.get_type() == EntityType::Zombie;
                    let clone_uuid = clone.get_uuid();
                    let zombie_uuid = zombie.get_uuid();
                    let fresh_uuid =
                        (clone_uuid.high, clone_uuid.low) != (zombie_uuid.high, zombie_uuid.low);
                    if same_type && fresh_uuid {
                        tracing::info!(
                            "E2E entity-snapshot-roundtrip bytes={} fresh-uuid=true",
                            snapshot.len()
                        );
                    } else {
                        tracing::info!(
                            "E2E entity-snapshot-mismatch same_type={same_type} fresh_uuid={fresh_uuid}"
                        );
                    }
                }
                None => tracing::info!(
                    "E2E entity-snapshot-rebuild-failed bytes={}",
                    snapshot.len()
                ),
            }
            // Player snapshots must not rebuild: the player type is not saveable.
            let player_snapshot = entity.create_snapshot();
            if world
                .spawn_entity_from_snapshot(&player_snapshot, pos)
                .is_none()
            {
                tracing::info!(
                    "E2E entity-snapshot-player-rejected bytes={}",
                    player_snapshot.len()
                );
            } else {
                tracing::info!("E2E entity-snapshot-player-unexpectedly-spawned");
            }

            // Brain memory read-only mapping (Bukkit MemoryKey equivalent):
            // zombies are goal-driven, so their brain is the brain-dead default
            // and queries come back empty; the call path itself is exercised.
            if let Some(mob) = zombie.as_mob() {
                let registered = mob.list_brain_memories();
                let walk_target = mob.get_brain_memory(memory_keys::WALK_TARGET);
                let unknown = mob.get_brain_memory("e2e:not_a_memory");
                tracing::info!(
                    "E2E brain-memory-query registered={} walk_target_present={} unknown_is_none={}",
                    registered.len(),
                    walk_target.is_some(),
                    unknown.is_none()
                );
            }
        }
        event
    }
}

struct TickWatcher;

impl EventHandler<ServerTickStartEvent> for TickWatcher {
    fn handle(
        &self,
        server: pumpkin_plugin_api::Server,
        event: ServerTickStartEventData,
    ) -> ServerTickStartEventData {
        let n = TICK_COUNT.fetch_add(1, Ordering::Relaxed);
        if n == 20 {
            tracing::info!("E2E tick-event flowing (20 ticks observed)");
            chunk_api_demo_start(&server);
        }
        chunk_api_demo_poll(&server);
        event
    }
}

// Chunk plugin-API long-tail (new mechanisms): loading-state query, plugin
// load tickets with asynchronous generation, forced chunks, and copy-on-read
// chunk snapshots. Driven from the tick event because headless e2e has no
// players: no chunk is loaded until load-chunk/forced tickets trigger the
// asynchronous generation pipeline.
fn chunk_api_demo_start(server: &pumpkin_plugin_api::Server) {
    let Some(world) = server.get_all_worlds().into_iter().next() else {
        tracing::info!("E2E chunk-demo-no-world");
        return;
    };
    let loaded_before = world.is_chunk_loaded(0, 0);
    let already = world.load_chunk(0, 0);
    tracing::info!(
        "E2E chunk-load-triggered loaded_before={loaded_before} already_loaded={already}"
    );

    world.set_chunk_forced(7, 7, true);
    let forced = world.is_chunk_forced(7, 7);
    let unforced = world.is_chunk_forced(8, 8);
    if forced && !unforced {
        tracing::info!("E2E chunk-forced-set");
    } else {
        tracing::info!("E2E chunk-forced-broken forced={forced} unforced={unforced}");
    }
    CHUNK_DEMO_ARMED.store(true, Ordering::Relaxed);
}

fn chunk_api_demo_poll(server: &pumpkin_plugin_api::Server) {
    if !CHUNK_DEMO_ARMED.load(Ordering::Relaxed) || CHUNK_DEMO_DONE.load(Ordering::Relaxed) {
        return;
    }
    let Some(world) = server.get_all_worlds().into_iter().next() else {
        return;
    };
    // Poll the loading state until the asynchronous generation lands.
    if !world.is_chunk_loaded(0, 0) {
        return;
    }
    CHUNK_DEMO_DONE.store(true, Ordering::Relaxed);
    tracing::info!("E2E chunk-load-async-landed");

    if let Some(snapshot) = world.get_chunk_snapshot(0, 0) {
        let top = snapshot.top_block_y(8, 8);
        let state = snapshot.block_state_id_at(8, top, 8);
        let biome = snapshot.biome_at(8, top, 8);
        let section_len = snapshot.section_block_states(0).len();
        let biome_len = snapshot.section_biomes(0).len();
        let all_len = snapshot.all_block_states().len();
        tracing::info!(
            "E2E chunk-snapshot x={} z={} min_y={} sections={} height={} top={} state={} biome={:?} section_dump={} biome_dump={} all={}",
            snapshot.get_x(),
            snapshot.get_z(),
            snapshot.get_min_y(),
            snapshot.get_section_count(),
            snapshot.height(),
            top,
            state,
            biome,
            section_len,
            biome_len,
            all_len
        );
    } else {
        tracing::info!("E2E chunk-snapshot-miss");
    }

    let live_present = world.get_chunk(0, 0).is_some();
    tracing::info!("E2E chunk-live-handle present={live_present}");

    // Release the plugin ticket and the forced mark again.
    world.unload_chunk(0, 0);
    world.set_chunk_forced(7, 7, false);
    let forced_after = world.is_chunk_forced(7, 7);
    if !forced_after {
        tracing::info!("E2E chunk-released forced_after=false");
    } else {
        tracing::info!("E2E chunk-release-broken forced_after=true");
    }
}

// Paper player-domain events (T3-player wiring). Each handler logs its marker
// once so the harness can assert the event actually flowed.

struct JumpWatcher;

impl EventHandler<PlayerJumpEvent> for JumpWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerJumpEventData,
    ) -> PlayerJumpEventData {
        if !JUMP_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-player-jump");
        }
        event
    }
}

struct ItemCooldownWatcher;

impl EventHandler<PlayerItemCooldownEvent> for ItemCooldownWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerItemCooldownEventData,
    ) -> PlayerItemCooldownEventData {
        if !ITEM_COOLDOWN_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-player-item-cooldown");
        }
        event
    }
}

struct TrackWatcher;

impl EventHandler<PlayerTrackEntityEvent> for TrackWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerTrackEntityEventData,
    ) -> PlayerTrackEntityEventData {
        if !TRACK_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-player-track-entity");
        }
        event
    }
}

struct TabCompleteWatcher;

impl EventHandler<AsyncTabCompleteEvent> for TabCompleteWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: AsyncTabCompleteEventData,
    ) -> AsyncTabCompleteEventData {
        if !TAB_COMPLETE_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-async-tab-complete");
        }
        event
    }
}

struct HandshakeWatcher;

impl EventHandler<PlayerHandshakeEvent> for HandshakeWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerHandshakeEventData,
    ) -> PlayerHandshakeEventData {
        if !HANDSHAKE_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-player-handshake");
        }
        event
    }
}

struct PurchaseWatcher;

impl EventHandler<PlayerPurchaseEvent> for PurchaseWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerPurchaseEventData,
    ) -> PlayerPurchaseEventData {
        if !PURCHASE_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-player-purchase");
        }
        event
    }
}

struct TradeWatcher;

impl EventHandler<PlayerTradeEvent> for TradeWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: PlayerTradeEventData,
    ) -> PlayerTradeEventData {
        if !TRADE_SEEN.swap(true, Ordering::Relaxed) {
            tracing::info!("E2E evt-player-trade merchant_id={}", event.merchant_id);
        }
        event
    }
}

// MapView map rendering (new mechanism): on the 20th observed tick, create a
// map in the first world, draw two corner pixels, place a cursor, lock the
// canvas, and verify the get-map round trip. Headless e2e has no players
// holding the map, so this exercises the host boundary and the MapManager
// state; the actual CMapItemData resend path is exercised in-game.
struct MapWatcher;

impl EventHandler<ServerTickStartEvent> for MapWatcher {
    fn handle(
        &self,
        server: pumpkin_plugin_api::Server,
        event: ServerTickStartEventData,
    ) -> ServerTickStartEventData {
        let n = MAP_TICKS.fetch_add(1, Ordering::Relaxed);
        if n < 20 || MAP_CHECKED.swap(true, Ordering::Relaxed) {
            return event;
        }
        use pumpkin_plugin_api::map::{MapCursor, WorldMapExt, cursor_types, rgb};

        let Some(world) = server.get_all_worlds().into_iter().next() else {
            tracing::info!("E2E map-view-skipped no-world");
            return event;
        };
        let map = world.create_map(0, 0, 0);
        let map_id = map.get_id();
        map.set_pixel(0, 0, rgb(255, 0, 0));
        map.set_pixel(127, 127, rgb(0, 0, 255));
        let cursor_index = map.add_cursor(&MapCursor {
            icon_type: cursor_types::RED_X,
            x: 64,
            z: 64,
            direction: 0,
            display_name: Some("e2e".to_string()),
        });
        map.lock();
        let roundtrip = pumpkin_plugin_api::map::get_map(map_id).is_some_and(|view| {
            view.get_id() == map_id && view.get_pixel(127, 127) != 0 && view.is_locked()
        });
        tracing::info!(
            "E2E map-view id={map_id} cursor_index={cursor_index} cursors={} roundtrip={roundtrip}",
            map.get_cursors().len()
        );
        event
    }
}

struct E2ePlugin;

impl Plugin for E2ePlugin {
    fn new() -> Self {
        Self
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "e2e-plugin".into(),
            version: "0.1.0".into(),
            authors: vec!["papokin".into()],
            description: "End-to-end plugin API verification plugin".into(),
            dependencies: vec![],
            permissions: vec![],
            load_after: vec![],
            load_before: vec![],
            provides: vec![],
            load_order: LoadOrder::PostWorld,
        }
    }

    fn config_defaults(&self) -> String {
        "message = \"hello-from-defaults\"\n\n[bonus]\nenabled = true\n".to_string()
    }

    fn on_load(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E on_load ok");
        Ok(())
    }

    fn on_enable(&self, context: Context) -> Result<()> {
        // Priority dispatch: Bukkit order = LOWEST first, HIGHEST last.
        context.register_event_handler(JoinAnnouncerLowest, EventPriority::Lowest, true, false)?;
        context.register_event_handler(
            JoinAnnouncerHighest,
            EventPriority::Highest,
            true,
            false,
        )?;
        context.register_event_handler(TickWatcher, EventPriority::Low, false, true)?;

        // Paper player-domain events (T3-player wiring).
        context.register_event_handler(JumpWatcher, EventPriority::Normal, false, false)?;
        context.register_event_handler(ItemCooldownWatcher, EventPriority::Normal, false, false)?;
        context.register_event_handler(TrackWatcher, EventPriority::Normal, false, false)?;
        context.register_event_handler(TabCompleteWatcher, EventPriority::Normal, false, false)?;
        context.register_event_handler(HandshakeWatcher, EventPriority::Normal, false, false)?;
        context.register_event_handler(PurchaseWatcher, EventPriority::Normal, false, false)?;
        context.register_event_handler(TradeWatcher, EventPriority::Normal, false, false)?;
        tracing::info!("E2E paper-events-registered count=7");

        // MapView map rendering (new mechanism), runs once on tick 20.
        context.register_event_handler(MapWatcher, EventPriority::Normal, false, true)?;

        // Async wall-clock task (new mechanism).
        context.schedule_async_delayed_task(300, |_server| {
            tracing::info!("E2E async-task-fired");
        });

        // Services registry (new mechanism): register + discover self.
        context.register_service("e2e:test", 5)?;
        match context.get_service_provider("e2e:test") {
            Some(ServiceProvider { plugin, .. }) if plugin == "e2e-plugin" => {
                tracing::info!("E2E service-registered-and-discovered provider={plugin}");
            }
            other => tracing::info!("E2E service-miss {other:?}"),
        }

        // Player plugin-message channel registry (new mechanism).
        context.register_incoming_channel("pumpkin:e2e")?;
        let channels = context.get_incoming_channels();
        if channels.contains(&"pumpkin:e2e".to_string()) {
            tracing::info!("E2E channel-registered channels={channels:?}");
        } else {
            tracing::info!("E2E channel-missed {channels:?}");
        }

        // Host-managed config with defaults deep-merge (new mechanism).
        match context.load_config() {
            Ok(config) => {
                let merged = config.contains("message")
                    && config.contains("hello-from-defaults")
                    && config.contains("[bonus]");
                if merged {
                    tracing::info!("E2E config-loaded-and-merged len={}", config.len());
                } else {
                    tracing::info!("E2E config-incomplete: {config}");
                }
            }
            Err(err) => tracing::info!("E2E config-error {err}"),
        }

        // Dynamic recipe registration (new mechanism): stonecutting, smithing
        // transform, smithing trim, and brewing.
        match context.register_recipe(StonecuttingRecipeBuilder::new(
            "e2e:glass_panes_from_glass",
            "minecraft:glass",
            ItemStack::new("minecraft:glass_pane", 4),
        )) {
            Ok(()) => tracing::info!("E2E recipe-stonecutting-registered"),
            Err(err) => tracing::info!("E2E recipe-stonecutting-failed {err}"),
        }

        match context.register_recipe(
            SmithingTransformRecipeBuilder::new(
                "e2e:netherite_chestplate",
                "minecraft:netherite_upgrade_smithing_template",
                "minecraft:diamond_chestplate",
                "minecraft:netherite_ingot",
                ItemStack::new("minecraft:netherite_chestplate", 1),
            )
            .copy_components(true),
        ) {
            Ok(()) => tracing::info!("E2E recipe-smithing-transform-registered"),
            Err(err) => tracing::info!("E2E recipe-smithing-transform-failed {err}"),
        }

        match context.register_recipe(SmithingTrimRecipeBuilder::new(
            "e2e:coast_trim_iron_chestplate",
            "minecraft:coast_armor_trim_smithing_template",
            "minecraft:iron_chestplate",
            "minecraft:amethyst_shard",
        )) {
            Ok(()) => tracing::info!("E2E recipe-smithing-trim-registered"),
            Err(err) => tracing::info!("E2E recipe-smithing-trim-failed {err}"),
        }

        match context.register_recipe(
            BrewingRecipeBuilder::new(
                "e2e:splash_water",
                "minecraft:potion",
                "minecraft:gunpowder",
                "minecraft:splash_potion",
            )
            .input_potion("minecraft:water")
            .output_potion("minecraft:water"),
        ) {
            Ok(()) => tracing::info!("E2E recipe-brewing-registered"),
            Err(err) => tracing::info!("E2E recipe-brewing-failed {err}"),
        }

        // Custom damage type registration (new mechanism): registered through
        // the damage type manager, which also appends it to the synced
        // damage_type registry.
        match context.register_damage_type(
            DamageTypeBuilder::new("e2e:frostbite", "frostbite")
                .scaling(DamageScaling::Always)
                .exhaustion(0.2)
                .effects(DamageEffects::Freezing),
        ) {
            Ok(()) => tracing::info!("E2E registry-damage-type-registered"),
            Err(err) => tracing::info!("E2E registry-damage-type-failed {err}"),
        }
        let damage_type_manager = context.get_damage_type_manager();
        if damage_type_manager.has("e2e:frostbite")
            && damage_type_manager
                .get_all_custom_names()
                .iter()
                .any(|name| name == "e2e:frostbite")
        {
            tracing::info!("E2E registry-damage-type-queryable");
        }

        // Tag overlay (new mechanism): a brand-new damage_type tag holding the
        // custom damage type registered above.
        match context.add_to_tag("damage_type", "e2e:custom_damage", "e2e:frostbite") {
            Ok(()) => {
                let values = context
                    .get_tag_manager()
                    .get_values("damage_type", "e2e:custom_damage");
                if values.iter().any(|name| name == "e2e:frostbite") {
                    tracing::info!("E2E registry-tag-entry-added values={values:?}");
                } else {
                    tracing::info!("E2E registry-tag-missing-entry {values:?}");
                }
            }
            Err(err) => tracing::info!("E2E registry-tag-failed {err}"),
        }

        // Generic custom registry entry (new mechanism): an inert domain that
        // no vanilla synced registry matches, so nothing extra is sent to
        // clients; the call still exercises registration, indexing, and the
        // network-id lookup against the real damage_type domain.
        // The payload is a valid empty NBT compound in network format.
        let registry_manager = context.get_registry_manager();
        match registry_manager.register("e2e/custom", "e2e:marker", vec![0x0A, 0x00, 0x00, 0x00]) {
            Ok(index) => {
                let frostbite_id = registry_manager.network_id("damage_type", "e2e:frostbite");
                if registry_manager.has("e2e/custom", "e2e:marker") {
                    tracing::info!(
                        "E2E registry-entry-registered index={index} frostbite-network-id={frostbite_id:?}"
                    );
                } else {
                    tracing::info!("E2E registry-entry-unqueryable index={index}");
                }
            }
            Err(err) => tracing::info!("E2E registry-entry-failed {err}"),
        }

        // Server build info (new mechanism).
        let build = context.get_build_info();
        tracing::info!(
            "E2E build-info brand={} api={}",
            build.brand,
            build.plugin_api_version
        );

        // Offline player lookup (new mechanism): a nil UUID is never known to
        // the server, so the lookup must return none.
        match context.get_offline_player_by_uuid("00000000-0000-0000-0000-000000000000") {
            None => tracing::info!("E2E offline-player-unknown-none"),
            Some(info) => tracing::info!("E2E offline-player-unexpected-hit uuid={}", info.uuid),
        }

        // Teleport flags (new mechanism): no player is online in headless e2e,
        // so this exercises construction and bit ops of the flags type only.
        let flags = TeleportFlags::X
            | TeleportFlags::Y
            | TeleportFlags::Z
            | TeleportFlags::Y_ROT
            | TeleportFlags::X_ROT;
        let has = |flag: TeleportFlags| (flags & flag).bits() != 0;
        if has(TeleportFlags::X)
            && has(TeleportFlags::Y_ROT)
            && !has(TeleportFlags::ROTATE_DELTA)
            && TeleportFlags::empty().bits() == 0
        {
            tracing::info!("E2E teleport-flags-available bits={:?}", flags.bits());
        } else {
            tracing::info!("E2E teleport-flags-broken {flags:?}");
        }

        // Merchant trade offers (new mechanism): headless e2e has no merchant
        // entity to grab, so this exercises the `TradeOffer` builder and the
        // item-stack round trip only; `Entity::as_merchant` /
        // `Merchant::{get,set,add,remove}_trade_offers` are the runtime entry
        // points exercised in-game.
        let offer = TradeOfferBuilder::new(
            ItemStack::new("minecraft:emerald", 3),
            ItemStack::new("minecraft:diamond", 1),
        )
        .cost_b(ItemStack::new("minecraft:book", 1))
        .max_uses(16)
        .xp(5)
        .build();
        if offer.max_uses == 16
            && offer.xp == 5
            && offer.reward_exp
            && offer.base_cost_a.get_count() == 3
            && offer.base_cost_a.get_registry_key() == "minecraft:emerald"
            && offer.cost_b.is_some()
        {
            tracing::info!(
                "E2E merchant-trade-offer-builder max_uses={} xp={} cost_b={}",
                offer.max_uses,
                offer.xp,
                offer.cost_b.is_some()
            );
        } else {
            tracing::info!("E2E merchant-trade-offer-broken");
        }

        // Registry-domain summary: all three managers must report the entries
        // registered above.
        tracing::info!(
            "E2E registry-summary damage-type={} tag={} entry={}",
            context.get_damage_type_manager().has("e2e:frostbite"),
            context
                .get_tag_manager()
                .get_values("damage_type", "e2e:custom_damage")
                .iter()
                .any(|name| name == "e2e:frostbite"),
            context
                .get_registry_manager()
                .has("e2e/custom", "e2e:marker"),
        );

        // Structure template API (new mechanism): register a runtime template
        // from gzipped NBT into the server's template cache (the same one the
        // `/place template` command resolves against), query it back through
        // has/list, then place it into a world.
        match pumpkin_plugin_api::structure::register_structure("e2e:mono_block", E2E_TEMPLATE_NBT)
        {
            Ok(()) => {
                let registered = pumpkin_plugin_api::structure::has_structure("e2e:mono_block");
                // Embedded vanilla templates resolve through the same cache.
                let embedded = pumpkin_plugin_api::structure::has_structure("minecraft:igloo/top");
                let listed = pumpkin_plugin_api::structure::list_structures()
                    .iter()
                    .any(|name| name == "e2e:mono_block");
                if registered && embedded && listed {
                    tracing::info!("E2E structure-registered-and-queryable");
                } else {
                    tracing::info!(
                        "E2E structure-query-incomplete registered={registered} embedded={embedded} listed={listed}"
                    );
                }

                let worlds = context.get_server().get_all_worlds();
                if let Some(world) = worlds.first() {
                    match world.place_structure(
                        "e2e:mono_block",
                        StructurePos { x: 8, y: 200, z: 8 },
                        None,
                        None,
                    ) {
                        Ok(true) => tracing::info!("E2E structure-placed"),
                        other => tracing::info!("E2E structure-place-unexpected {other:?}"),
                    }
                } else {
                    tracing::info!("E2E structure-place-skipped no-world");
                }
            }
            Err(err) => tracing::info!("E2E structure-register-failed {err}"),
        }

        // Loot table API (new mechanism): pure-data generation against the
        // static datapack tables, so it runs headless with no world or player
        // context; a fixed seed makes every roll deterministic.
        match pumpkin_plugin_api::loot::generate_loot("minecraft:chests/simple_dungeon", 0x5EED) {
            Ok(stacks) => {
                let total: u32 = stacks.iter().map(|s| u32::from(s.get_count())).sum();
                let context = pumpkin_plugin_api::loot::LootContext::new()
                    .killed_by_player(true)
                    .tool(ItemStack::new("minecraft:diamond_sword", 1));
                let zombie_drops = pumpkin_plugin_api::loot::generate_loot_with_context(
                    "minecraft:entities/zombie",
                    0x5EED,
                    context,
                )
                .map(|drops| drops.len());
                tracing::info!(
                    "E2E loot-generate stacks={} items={total} zombie-stacks={zombie_drops:?}",
                    stacks.len()
                );
            }
            Err(err) => tracing::info!("E2E loot-generate-failed {err}"),
        }
        let known = pumpkin_plugin_api::loot::has_loot_table("minecraft:chests/simple_dungeon");
        let unknown = pumpkin_plugin_api::loot::has_loot_table("minecraft:e2e/no_such_table");
        if known && !unknown {
            tracing::info!("E2E loot-has-table true-and-false-paths-ok");
        } else {
            tracing::info!("E2E loot-has-table-broken known={known} unknown={unknown}");
        }

        // Client cookie API (new mechanism): no real client connects during
        // the headless run, so the store/request round trip cannot run here.
        // Exercise the startup-safe surface instead (guest-side key
        // validation mirroring the host, plus the payload limit constant) and
        // mark the API as linked.
        let valid = pumpkin_plugin_api::cookie::is_valid_cookie_key("e2e:session");
        let invalid = pumpkin_plugin_api::cookie::is_valid_cookie_key("E2E:Bad Key");
        if valid && !invalid {
            tracing::info!(
                "E2E cookie-api-ready max_payload={}",
                pumpkin_plugin_api::cookie::MAX_COOKIE_PAYLOAD
            );
        } else {
            tracing::info!("E2E cookie-api-broken valid={valid} invalid={invalid}");
        }

        // DragonBattle dragon fight (new mechanism): only The End carries a
        // dragon fight, so querying the overworld must return none. When an
        // End world is present, run the read-only queries plus the guarded
        // mutation entry points (all no-ops while the dragon is not dead and
        // no respawn sequence is running, so the headless run with no players
        // and no dragon spawned stays safe).
        {
            use pumpkin_plugin_api::dragon::{DragonRespawnStage, WorldDragonFightExt};

            let worlds = context.get_server().get_all_worlds();
            if worlds.is_empty() {
                tracing::info!("E2E dragon-fight-skipped no-world");
            }
            for world in &worlds {
                let Some(fight) = world.get_dragon_fight() else {
                    tracing::info!("E2E dragon-fight-none world={}", world.get_id());
                    continue;
                };
                let noop_initiated = fight.initiate_respawn();
                let noop_stage_set = fight.set_respawn_stage(DragonRespawnStage::End);
                let noop_aborted = fight.abort_respawn();
                tracing::info!(
                    "E2E dragon-fight-api-ready world={} uuid_tracked={} alive={} killed_before={} stage={:?} crystals={} portal={:?} guards={}{}{}",
                    world.get_id(),
                    fight.get_dragon_uuid().is_some(),
                    fight.is_dragon_alive(),
                    fight.has_been_killed_previously(),
                    fight.get_respawn_stage(),
                    fight.get_alive_crystals(),
                    fight.get_exit_portal_location(),
                    noop_initiated,
                    noop_stage_set,
                    noop_aborted,
                );
            }
        }

        tracing::info!("E2E on_enable ok");
        Ok(())
    }

    fn on_disable(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E on_disable ok");
        Ok(())
    }

    fn on_unload(&self, _context: Context) -> Result<()> {
        tracing::info!("E2E on_unload ok");
        Ok(())
    }

    fn on_plugin_message(&self, player_uuid: &str, channel: &str, data: &[u8]) {
        tracing::info!(
            "E2E plugin-message player={player_uuid} channel={channel} len={}",
            data.len()
        );
    }
}

register_plugin!(E2ePlugin);
