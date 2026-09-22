use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

use super::{goal_utils, random_pos};
use crate::entity::ai::pathfinder::pathfinding_context::PathfindingContext;
use crate::entity::mob::Mob;
use crate::world::World;

/// 类似 [`super::default_random_pos::get_pos`]，但候选位置会被向上推出固体方块
/// 而水会被拒绝。
#[must_use]
pub fn get_pos(mob: &dyn Mob, horizontal_dist: i32, vertical_dist: i32) -> Option<Vector3<f64>> {
    let restrict = goal_utils::mob_restricted(mob, f64::from(horizontal_dist));
    let world = mob.get_entity().world.load_full();
    let block_pos = mob.get_entity().block_pos.load();
    let mut context = PathfindingContext::new(
        Vector3::new(block_pos.0.x, block_pos.0.y, block_pos.0.z),
        world.clone(),
    );
    let mut rng = mob.get_random();

    random_pos::generate_random_pos(
        || {
            let direction =
                random_pos::generate_random_direction(&mut rng, horizontal_dist, vertical_dist);
            let pos = toward_direction(
                mob,
                &world,
                f64::from(horizontal_dist),
                restrict,
                direction,
                &mut rng,
            )?;
            move_pos_up_out_of_solid(mob, &world, &mut context, pos)
        },
        |pos| f64::from(mob.get_walk_target_value(pos)),
    )
}

#[must_use]
pub fn move_pos_up_out_of_solid(
    mob: &dyn Mob,
    world: &World,
    context: &mut PathfindingContext,
    pos: BlockPos,
) -> Option<BlockPos> {
    let max_y = world.dimension.min_y + world.dimension.height - 1;
    let pos = random_pos::move_up_out_of_solid(pos, max_y, |candidate| {
        goal_utils::is_solid(world, candidate)
    });
    (!goal_utils::is_water(world, &pos) && !goal_utils::has_malus(mob, context, &pos))
        .then_some(pos)
}

/// 与默认变体不同，减益（malus）检查留给 [`move_pos_up_out_of_solid`] 处理。
pub fn toward_direction(
    mob: &dyn Mob,
    world: &World,
    horizontal_dist: f64,
    restrict: bool,
    direction: Vector3<i32>,
    rng: &mut impl rand::RngExt,
) -> Option<BlockPos> {
    let pos =
        random_pos::generate_random_pos_toward_direction(mob, horizontal_dist, rng, direction);
    (!goal_utils::is_outside_limits(&pos, world)
        && !goal_utils::is_restricted(restrict, mob, &pos)
        && !goal_utils::is_not_stable(world, &pos))
    .then_some(pos)
}
