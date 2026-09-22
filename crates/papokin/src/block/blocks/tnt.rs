use papokin_data::Block;
use papokin_data::BlockStateId;
use papokin_data::block_properties::TntLikeProperties;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::sound::SoundCategory;
use papokin_data::translation;
use papokin_macros::pumpkin_block;
use papokin_util::GameMode;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;
use papokin_world::world::BlockFlags;
use rand::RngExt;
use std::sync::Arc;

use super::redstone::block_receives_redstone_power;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, ExplodeArgs, OnNeighborUpdateArgs, OnProjectileHitArgs, PlacedArgs,
    UseWithItemArgs,
};
use crate::entity::Entity;
use crate::entity::tnt::TNTEntity;
use crate::world::World;

#[pumpkin_block("minecraft:tnt")]
pub struct TNTBlock;

const DEFAULT_FUSE: u32 = 80;
const DEFAULT_POWER: f32 = 4.0;

impl TNTBlock {
    pub fn prime(world: &Arc<World>, location: &BlockPos, reason: &str) -> bool {
        if !world.level_info.load().game_rules.tnt_explodes {
            return false;
        }

        let mut event = crate::plugin::api::events::block::tnt_prime::TNTPrimeEvent::new(
            *location,
            reason.to_string(),
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return false;
        }

        let spawn_pos = Vector3::new(
            location.0.x as f64 + 0.5,
            location.0.y as f64,
            location.0.z as f64 + 0.5,
        );
        let entity = Entity::new(world.clone(), spawn_pos, &EntityType::TNT);
        let mut prime_event =
            crate::plugin::api::events::entity::explosion_prime::ExplosionPrimeEvent::new(
                entity.entity_id,
                DEFAULT_POWER,
                false,
            );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut prime_event);
        }
        if prime_event.cancelled {
            return false;
        }

        let tnt = Arc::new(TNTEntity::new(entity, DEFAULT_POWER, DEFAULT_FUSE));
        world.spawn_entity(tnt);
        world.play_sound(
            papokin_data::sound::Sound::EntityTntPrimed,
            SoundCategory::Blocks,
            &spawn_pos,
        );
        world.set_block_state(location, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
        true
    }
}

impl BlockBehaviour for TNTBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let item_id = args.item_stack.item.id;
        if item_id != Item::FLINT_AND_STEEL.id && item_id != Item::FIRE_CHARGE.id {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        if Self::prime(args.world, args.position, "PLAYER_IGNITE") {
            if args.player.gamemode.load() != GameMode::Creative {
                if item_id == Item::FLINT_AND_STEEL.id {
                    let _ = args.item_stack.damage_item(1);
                } else {
                    args.item_stack.decrement(1);
                }
            }
            BlockActionResult::Success
        } else if !args.world.level_info.load().game_rules.tnt_explodes {
            args.player.send_system_message_raw(
                &TextComponent::translate(translation::java::BLOCK_MINECRAFT_TNT_DISABLED, []),
                true,
            );
            BlockActionResult::Pass
        } else {
            BlockActionResult::Success
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block != Block::from_state_id(args.old_state_id)
            && block_receives_redstone_power(args.world, args.position)
        {
            Self::prime(args.world, args.position, "REDSTONE");
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if block_receives_redstone_power(args.world, args.position) {
            Self::prime(args.world, args.position, "REDSTONE");
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            let props = TntLikeProperties::from_state_id(args.state.id);
            if props.r#unstable {
                Self::prime(args.world, args.position, "EXPLOSION");
            }
        }
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        if args.projectile.get_entity().is_on_fire() {
            Self::prime(args.world, args.position, "FIRE");
        }
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        if !args.world.level_info.load().game_rules.tnt_explodes {
            return;
        }

        let mut event = crate::plugin::api::events::block::tnt_prime::TNTPrimeEvent::new(
            *args.position,
            "EXPLOSION".to_string(),
        );
        if let Some(server) = args.world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        let spawn_pos = Vector3::new(
            args.position.0.x as f64 + 0.5,
            args.position.0.y as f64,
            args.position.0.z as f64 + 0.5,
        );
        let entity = Entity::new(args.world.clone(), spawn_pos, &EntityType::TNT);
        let fuse = rand::rng().random_range(0..DEFAULT_FUSE / 4) + DEFAULT_FUSE / 8;
        let tnt = Arc::new(TNTEntity::new(entity, DEFAULT_POWER, fuse));
        args.world.spawn_entity(tnt);
    }

    fn should_drop_items_on_explosion(&self) -> bool {
        false
    }
}
