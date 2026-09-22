#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SChangeDifficulty;

impl JavaClient {
    pub fn handle_change_difficulty(
        &self,
        server: &Server,
        player: &Player,
        packet: &SChangeDifficulty,
    ) {
        if player.permission_lvl.load() < PermissionLvl::Two {
            warn!(
                "玩家 {} 试图更改难度，但缺少所需权限",
                player.gameprofile.name
            );
            return;
        }

        let current_info = server.level_info.load();
        if current_info.difficulty_locked {
            warn!(
                "玩家 {} 试图在难度已锁定时更改难度",
                player.gameprofile.name
            );
            return;
        }

        server.set_difficulty(packet.difficulty, false);

        info!(
            "玩家 {} 将难度更改为 {:?}",
            player.gameprofile.name, packet.difficulty
        );
    }
}
