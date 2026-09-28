#[allow(clippy::wildcard_imports)]
use super::*;
use crate::entity::vehicle::minecart::MinecartEntity;
use papokin_protocol::java::server::play::SSetCommandMinecart;

impl JavaClient {
    pub fn handle_set_command_minecart(&self, player: &Player, packet: &SSetCommandMinecart<'_>) {
        // 原版 canUseGameMasterBlocks()：命令方块矿车界面仅创造模式可用，
        // 生存模式的 OP2 同样不允许（与 handle_set_command_block 对齐）。
        if !player.is_creative() || player.permission_lvl.load() < PermissionLvl::Two {
            return;
        }

        let world = player.world();
        let Some(entity) = world.get_entity_by_id(packet.entity_id.0) else {
            return;
        };
        // 仅命令方块矿车可被此包更新，其余实体一律忽略
        let Some(minecart) = entity.cast_any().downcast_ref::<MinecartEntity>() else {
            return;
        };
        let Some(command_cart) = minecart.command_block() else {
            return;
        };

        let mut command = packet.command;
        if let Some(stripped) = command.strip_prefix('/') {
            command = stripped;
        }
        command_cart.set_command(command);
        debug!(
            "玩家 {} 将命令方块矿车 {} 的命令更新为：{}",
            player.gameprofile.name,
            entity.get_entity().entity_id,
            packet.command
        );
    }
}
