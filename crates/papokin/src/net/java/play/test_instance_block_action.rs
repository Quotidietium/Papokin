#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_test_instance_block_action(
        &self,
        player: &Arc<Player>,
        packet: &STestInstanceBlockAction<'_>,
    ) {
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();
        debug!(
            "测试实例方块动作位于 {:?}：action={:?}",
            packet.pos, packet.action
        );
    }
}
