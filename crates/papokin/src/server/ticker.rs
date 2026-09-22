use super::server_test_manager::drain_game_test_queue;

use crate::{
    STOP_INTERRUPT,
    plugin::server::{
        server_tick_end::ServerTickEndEvent, server_tick_start::ServerTickStartEvent,
    },
    server::Server,
};
use papokin_gametest::GameTestRunner;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tracing::debug;

pub struct Ticker;

impl Ticker {
    /// 在专用线程上运行服务器主刻循环。
    pub fn run(server: &Arc<Server>) {
        let _guard = server.runtime.enter();
        let mut next_tick = Instant::now();
        let mut game_test_runner = GameTestRunner::new();

        'ticker: loop {
            let tick_start_time = Instant::now();
            let manager = &server.tick_rate_manager;

            manager.tick();

            let tick_number = server.tick_count.load(Ordering::Relaxed);
            if server.plugin_manager.has_handlers::<ServerTickStartEvent>() {
                server.runtime.block_on(
                    server
                        .plugin_manager
                        .fire(server, &mut ServerTickStartEvent::new(tick_number)),
                );
            }

            let should_tick_game_tests = manager.runs_normally() || manager.is_sprinting();

            if manager.is_sprinting() {
                manager.start_sprint_tick_work();
                server.tick();

                if manager.end_sprint_tick_work() {
                    manager.finish_tick_sprint(server);
                }
            } else {
                server.tick();
            }

            if should_tick_game_tests {
                server.runtime.block_on(async {
                    drain_game_test_queue(server, &mut game_test_runner).await;
                    game_test_runner.tick().await;
                });
            }

            let tick_duration_nanos = tick_start_time.elapsed().as_nanos() as i64;

            let tick_number = server.tick_count.load(Ordering::Relaxed);
            if server.plugin_manager.has_handlers::<ServerTickEndEvent>() {
                server.runtime.block_on(server.plugin_manager.fire(
                    server,
                    &mut ServerTickEndEvent::new(tick_number, tick_duration_nanos),
                ));
            }

            server.update_tick_times(tick_duration_nanos);

            let tick_interval = if manager.is_sprinting() {
                Duration::ZERO
            } else {
                Duration::from_nanos(manager.nanoseconds_per_tick() as u64)
            };

            next_tick += tick_interval;

            if STOP_INTERRUPT.is_cancelled() {
                break 'ticker;
            }

            let now = Instant::now();
            if next_tick > now {
                let sleep_duration = next_tick - now;
                let cancelled = STOP_INTERRUPT.clone();
                server.runtime.block_on(async {
                    tokio::select! {
                        () = tokio::time::sleep(sleep_duration) => {},
                        () = cancelled.cancelled() => {},
                    }
                });

                if STOP_INTERRUPT.is_cancelled() {
                    break 'ticker;
                }
            }

            // 死亡螺旋预防 / 追赶钳制
            // 如果服务器落后于计划刻，将 next_tick 钳制到当前
            // 这样我们就不会以零睡眠连跑一串刻。
            let now = Instant::now();
            if now > next_tick {
                next_tick = now;
            }
        }

        debug!("计时器已停止");
    }
}
