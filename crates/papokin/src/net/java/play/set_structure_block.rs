#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SSetStructureBlock;

impl JavaClient {
    pub fn handle_set_structure_block(&self, player: &Player, packet: &SSetStructureBlock<'_>) {
        if player.permission_lvl.load() < PermissionLvl::Two {
            return;
        }

        debug!(
            "玩家 {} 在 {:?} 设置结构方块，名称：{}，模式：{}",
            player.gameprofile.name, packet.location, packet.name, packet.mode.0
        );
    }
}
