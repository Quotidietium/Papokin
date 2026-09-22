#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SLockDifficulty;

impl JavaClient {
    pub fn handle_lock_difficulty(
        &self,
        server: &Server,
        player: &Player,
        packet: &SLockDifficulty,
    ) {
        if player.permission_lvl.load() >= PermissionLvl::Two {
            server.set_difficulty_locked(packet.locked);
            info!(
                "玩家 {} 锁定了难度：{}",
                player.gameprofile.name, packet.locked
            );
        } else {
            warn!(
                "玩家 {} 试图锁定难度，但缺少所需权限",
                player.gameprofile.name
            );
        }
    }
}
