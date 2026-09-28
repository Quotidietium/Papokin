#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_attack(&self, player: &Arc<Player>, attack: &SAttack, server: &Arc<Server>) {
        if !player.has_client_loaded() {
            return;
        }
        // 旁观者无敌但自身不可参与战斗：与 handle_interact 的旁观分支
        // 对齐，防止改过的客户端借旁观模式攻击实体。
        if player.gamemode.load() == GameMode::Spectator {
            return;
        }
        player.update_last_action_time();
        let entity_id = attack.entity_id;
        let player_entity = &player.get_entity();
        let world = player_entity.world.load_full();

        let config = &server.advanced_config.pvp;
        if !config.enabled {
            return;
        }

        if entity_id.0 == player.entity_id() {
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_INVALID_ENTITY_ATTACKED,
                [],
            ));
            return;
        }

        let player_target = world.get_player_by_id(entity_id.0);
        let target: Option<Arc<dyn EntityBase>> = player_target
            .as_ref()
            .map(|p| Arc::clone(p) as Arc<dyn EntityBase>)
            .or_else(|| world.get_entity_by_id(entity_id.0));
        let Some(target) = target else {
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_INVALID_ENTITY_ATTACKED,
                [],
            ));
            return;
        };
        // 交互距离稽查已移除：只对被修改的客户端生效，属反作弊插件
        // 职责（插件可经实体交互事件自行校验距离后取消）。
        if let Some(player_victim) = &player_target {
            if player_victim.living_entity.health.load() <= 0.0 {
                return;
            }
            if config.protect_creative && player_victim.gamemode.load() == GameMode::Creative {
                world.play_sound(
                    Sound::EntityPlayerAttackNodamage,
                    SoundCategory::Players,
                    &player_victim.position(),
                );
                return;
            }
        }
        player.attack(&target);
    }
}
