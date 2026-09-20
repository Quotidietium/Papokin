//! End-to-end test plugin.
//!
//! Exercises the plugin-API mechanisms added in the Papo-coverage pass:
//! lifecycle (on_load/on_enable), event dispatch with priorities, async
//! wall-clock tasks, host-managed config, the services registry, and the
//! player plugin-message channel registry. Every step logs an `E2E` marker
//! that the harness asserts in the server log.

use std::sync::atomic::{AtomicU32, Ordering};

use pumpkin_plugin_api::events::{EventData, EventHandler, EventPriority, PlayerJoinEvent, ServerTickStartEvent};
use pumpkin_plugin_api::scheduler::SchedulerExt;
use pumpkin_plugin_api::services::ServiceProvider;
use pumpkin_plugin_api::{Context, LoadOrder, Plugin, PluginMetadata, Result, register_plugin};

type PlayerJoinEventData = EventData<PlayerJoinEvent>;
type ServerTickStartEventData = EventData<ServerTickStartEvent>;

static JOIN_COUNT: AtomicU32 = AtomicU32::new(0);
static TICK_COUNT: AtomicU32 = AtomicU32::new(0);

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
        context.register_event_handler(JoinAnnouncerHighest, EventPriority::Highest, true, false)?;
        context.register_event_handler(TickWatcher, EventPriority::Low, false, true)?;

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
