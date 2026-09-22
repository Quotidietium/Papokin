#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_client_tick_end(&self, player: &Player) {
        // 如果本刻没有收到移动数据包，则玩家处于静止状态——
        // 将已知移动量清零，让使用方（例如激流速度）看到 0。
        // 与原版 ServerGamePacketListenerImpl#handleClientTickEnd 一致。
        if !self
            .received_movement_this_tick
            .swap(false, Ordering::Relaxed)
        {
            player
                .get_entity()
                .movement
                .store(Vector3::new(0.0, 0.0, 0.0));
        }
    }
}
