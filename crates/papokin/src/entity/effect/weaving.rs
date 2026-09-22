use papokin_data::Block;
use papokin_data::damage::DamageType;
use papokin_data::world::WorldEvent;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_world::world::BlockFlags;
use rustc_hash::FxHashSet;

use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct WeavingMobEffect;

impl MobEffect for WeavingMobEffect {
    fn on_mob_death(&self, living: &LivingEntity, _amplifier: u8, _damage_type: &DamageType) {
        let world = living.entity.world.load();

        // 检查是否启用生物破坏或玩家
        let mob_griefing = world.level_info.load().game_rules.mob_griefing;
        if !living.is_player() && !mob_griefing {
            return;
        }

        let center_pos = living.entity.block_pos.load();
        let cobweb_count = (rand::random::<u32>() % 2 + 2) as usize; // 2 至 3 个蜘蛛网

        let mut positions_to_transform = FxHashSet::default();

        // 在半径为 1 的立方体内采样至多 15 个随机位置
        for _ in 0..15 {
            let dx = (rand::random::<u32>() % 3) as i32 - 1;
            let dy = (rand::random::<u32>() % 3) as i32 - 1;
            let dz = (rand::random::<u32>() % 3) as i32 - 1;

            let target_pos = BlockPos(center_pos.0 + Vector3::new(dx, dy, dz));
            let below_pos = BlockPos(target_pos.0 + Vector3::new(0, -1, 0));

            let target_state = world.get_block_state(&target_pos);
            let below_state = world.get_block_state(&below_pos);

            if target_state.is_air()
                && !below_state.is_air()
                && positions_to_transform.insert(target_pos)
                && positions_to_transform.len() >= cobweb_count
            {
                break;
            }
        }

        for target_pos in positions_to_transform {
            world.set_block_state(
                &target_pos,
                Block::COBWEB.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            world.sync_world_event(WorldEvent::AnimationSpawnCobweb, target_pos, 0);
        }
    }
}
