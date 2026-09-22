#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_player_ground(&self, player: &Player, ground: &SSetPlayerGround) {
        // 本刻收到了移动数据包 — 用于 SClientTickEnd 归零跟踪。
        self.received_movement_this_tick
            .store(true, Ordering::Relaxed);
        player
            .living_entity
            .entity
            .on_ground
            .store(ground.on_ground, Ordering::Relaxed);
    }
}
