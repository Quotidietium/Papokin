use std::sync::Arc;

use papokin_data::entity::EntityType;
use papokin_macros::pumpkin_block_from_tag;
use papokin_util::GameMode;

use crate::block::BlockBehaviour;
use crate::block::BrokenArgs;
use crate::entity::Entity;

#[pumpkin_block_from_tag("c:cobblestones/infested")]
pub struct InfestedBlock;

impl BlockBehaviour for InfestedBlock {
    fn broken(&self, args: BrokenArgs<'_>) {
        {
            // TODO: 权宜的丑陋修复，应改用 onStacksDropped
            if args.player.gamemode.load() == GameMode::Creative {
                return;
            }
            let entity = Entity::new(
                args.world.clone(),
                args.position.0.to_f64(),
                &EntityType::SILVERFISH,
            );

            args.world.spawn_entity(Arc::new(entity));
        }
    }
}
