use std::sync::Arc;

use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;

use super::EnchantmentEntityEffectExt;
use crate::command::CommandSender;
use crate::command::context::command_source::CommandSource;
use crate::entity::Entity;
use crate::entity::player::Player;
use crate::world::World;

/// 执行数据包函数的附魔实体效果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFunction {
    pub function: String,
}

impl RunFunction {
    #[must_use]
    pub fn new(function: impl Into<String>) -> Self {
        Self {
            function: function.into(),
        }
    }

    pub fn apply(
        &self,
        world: &Arc<World>,
        position: Vector3<f64>,
        owner: Option<&Arc<Player>>,
        entity: Option<&Entity>,
    ) {
        let Some(server) = world.server.upgrade() else {
            return;
        };

        let rotation = entity.map_or_else(Vector2::default, |e| {
            Vector2::new(e.yaw.load(), e.pitch.load())
        });

        let source = owner.map_or_else(
            || {
                let mut src = CommandSource::new(
                    CommandSender::Console,
                    world.clone(),
                    None,
                    position,
                    rotation,
                    "Enchantment".to_string(),
                    TextComponent::text("附魔"),
                    server.clone(),
                );
                src.silent = true;
                src
            },
            |player| {
                let mut src = player.get_command_source(&server);
                src.position = position;
                src.rotation = rotation;
                src.silent = true;
                src
            },
        );

        if let Err(err) = server
            .datapack_manager
            .execute_function(&server, &source, &self.function)
        {
            tracing::error!(
                "附魔 run_function 效果失败，函数 {} 不存在：{err}",
                self.function
            );
        }
    }
}

impl EnchantmentEntityEffectExt for RunFunction {
    fn apply(
        &self,
        world: &Arc<World>,
        _enchantment_level: i32,
        owner: Option<&Arc<Player>>,
        entity: Option<&Entity>,
        position: Vector3<f64>,
    ) {
        self.apply(world, position, owner, entity);
    }
}
