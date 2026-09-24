#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SSpectateEntity;

impl JavaClient {
    pub fn handle_teleport_to_entity(
        &self,
        player: &Arc<Player>,
        packet: &STeleportToEntity,
        server: &Server,
    ) {
        // 26.2+ 协议将旁观包从 SpectateEntity 改名重发为 TeleportToEntity
        // （1.21.11 及以下仍发 SpectateEntity），两者字段一致、语义相同，
        // 必须走同一条旁观处理路径：否则 26.x 客户端不触发
        // `PlayerStartSpectatingEntityEvent`、也无法旁观非玩家实体，
        // 造成随客户端版本分裂的行为差异
        self.handle_spectate_entity(
            player,
            server,
            &SSpectateEntity {
                target: packet.target,
            },
        );
    }
}
