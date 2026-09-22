#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SPlayPong;

impl JavaClient {
    pub fn handle_play_pong(&self, player: &Player, packet: &SPlayPong) {
        debug!(
            "已收到来自玩家 {} 的 pong，id {}",
            player.gameprofile.name, packet.id
        );
    }
}
