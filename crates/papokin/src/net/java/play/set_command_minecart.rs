#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SSetCommandMinecart;

impl JavaClient {
    pub fn handle_set_command_minecart(&self, player: &Player, packet: &SSetCommandMinecart<'_>) {
        if player.permission_lvl.load() < PermissionLvl::Two {
            return;
        }

        let world = player.world();
        if let Some(entity) = world.get_entity_by_id(packet.entity_id.0) {
            debug!(
                "玩家 {} 将命令方块矿车 {} 的命令更新为：{}",
                player.gameprofile.name,
                entity.get_entity().entity_id,
                packet.command
            );
        }
    }
}
