#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_close_container(&self, player: &Arc<Player>, packet: &SCloseContainer) {
        // 原版会校验窗口 id：与当前界面不符的关闭包直接忽略，
        // 防止伪造包关掉玩家并未在操作的其他界面
        let current_sync_id = {
            let handler_arc = player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            handler_arc
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get_behaviour()
                .sync_id
        };
        if packet.window_id.0 != i32::from(current_sync_id) {
            return;
        }
        player.on_handled_screen_closed();
    }
}
