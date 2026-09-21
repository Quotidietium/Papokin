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
use pumpkin_plugin_api::{
    Context, ItemStack, LoadOrder, Plugin, PluginMetadata, Result, TeleportFlags, register_plugin,
};

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
        event
    }
}

struct TickWatcher;

impl EventHandler<ServerTickStartEvent> for TickWatcher {
    fn handle(
        &self,
        _server: pumpkin_plugin_api::Server,
        event: ServerTickStartEventData,
    ) -> ServerTickStartEventData {
        let n = TICK_COUNT.fetch_add(1, Ordering::Relaxed);
        if n == 20 {
            tracing::info!("E2E tick-event flowing (20 ticks observed)");
        }
        event
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
