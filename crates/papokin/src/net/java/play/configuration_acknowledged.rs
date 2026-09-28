#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_configuration_acknowledged(&self, player: &Player) {
        // 该包仅在服务端先发送 CStartConfiguration（重配置请求）后才合法，
        // 而本服务端从不主动发送它。因此在 play 阶段收到必为伪造包：
        // 无条件接受会把连接状态翻转到 Config，破坏后续断开包的
        // 语义（kick 会按配置阶段发包，客户端无法解析）。
        warn!(
            "玩家 {} 在未请求重配置的情况下发送了配置确认包，已断开",
            player.gameprofile.name
        );
        self.try_kick(&TextComponent::translate(
            translation::java::MULTIPLAYER_DISCONNECT_INVALID_PACKET,
            [],
        ));
    }
}
