#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::ConnectionState;

impl JavaClient {
    pub fn handle_configuration_acknowledged(&self, player: &Player) {
        debug!("玩家 {} 确认了配置切换", player.gameprofile.name);
        self.connection_state.store(ConnectionState::Config);
    }
}
