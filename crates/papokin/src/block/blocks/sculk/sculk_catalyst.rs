use std::sync::Arc;

use papokin_data::tag::{self, Taggable};
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockFlags;
use rand::RngExt;

use crate::block::{BlockBehaviour, BlockMetadata, OnPlaceArgs};
use crate::world::World;
use papokin_data::block_properties::{
    SculkCatalystLikeProperties, SculkSensorLikeProperties, SculkShriekerLikeProperties,
};
use papokin_data::{Block, BlockId, BlockStateId};

pub struct SculkCatalystBlock;

impl BlockMetadata for SculkCatalystBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_CATALYST].into()
    }
}

impl SculkCatalystBlock {
    /// 原版幽匿催发：生物在催化体 8 格内死亡时，其死亡经验被催化体吸收，
    /// 作为充能在周围催发幽匿块；充能随传播衰减（每格 1 点）。
    /// 返回是否成功催发；催发后死亡经验不再生成经验球。
    pub fn absorb_death(world: &Arc<World>, catalyst: BlockPos, charge: u32) -> bool {
        let mut rng = rand::rng();
        let mut charge = charge.min(1000);
        let mut placed = 0u32;

        // 催发主体：在催化体周围随机挑选 `#sculk_replaceable` 方块转为幽匿块。
        // 简化说明：原版通过幽匿脉作为充能路径逐步传播，这里直接随机落点，
        // 玩家可见结果一致（幽匿块在催化体周围蔓延、经验被吸收）。
        for _ in 0..64 {
            if charge == 0 {
                break;
            }
            let target = BlockPos::new(
                catalyst.0.x + rng.random_range(-8i32..=8),
                catalyst.0.y + rng.random_range(-4i32..=4),
                catalyst.0.z + rng.random_range(-8i32..=8),
            );
            if target == catalyst {
                continue;
            }
            if !Self::try_bloom_at(world, target, charge as i32) {
                continue;
            }
            charge -= 1;
            placed += 1;
        }

        if placed > 0 {
            // 充能充足时小概率催发尖叫体（原版 1%；can_summon=false，不会召唤监守者）
            if charge >= 10 && rng.random_range(0..100) < 1 {
                let mut props = SculkShriekerLikeProperties::default(&Block::SCULK_SHRIEKER);
                props.can_summon = false;
                Self::place_feature_block(
                    world,
                    catalyst,
                    charge as i32,
                    props.to_state_id(&Block::SCULK_SHRIEKER),
                );
            }
            // 小概率催发传感器（原版 1%）
            if rng.random_range(0..100) < 1 {
                let props = SculkSensorLikeProperties::default(&Block::SCULK_SENSOR);
                Self::place_feature_block(
                    world,
                    catalyst,
                    charge as i32,
                    props.to_state_id(&Block::SCULK_SENSOR),
                );
            }
        }

        placed > 0
    }

    /// 尝试把一个位置催发为幽匿块：仅 `#sculk_replaceable` 且上方为空气/液体；
    /// `SculkBloom` 事件取消则放弃该催发点。
    fn try_bloom_at(world: &Arc<World>, target: BlockPos, charge: i32) -> bool {
        let (block, _) = world.get_block_and_state_id(&target);
        if !block.has_tag(&tag::Block::MINECRAFT_SCULK_REPLACEABLE) {
            return false;
        }
        let above_state = world.get_block_state(&target.up());
        if !(above_state.is_air() || above_state.is_liquid()) {
            return false;
        }

        let mut event = crate::plugin::api::events::block::sculk_bloom::SculkBloomEvent::new(
            target,
            world.clone(),
            charge,
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return false;
            }
        }

        world.set_block_state(
            &target,
            Block::SCULK.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        true
    }

    /// 在催化体周围找一个可替换位置放置特色方块（尖叫体/传感器），
    /// 放置前同样触发 `SculkBloom` 事件，取消则放弃该候选位置。
    fn place_feature_block(
        world: &Arc<World>,
        catalyst: BlockPos,
        charge: i32,
        state: BlockStateId,
    ) {
        let mut rng = rand::rng();
        for _ in 0..8 {
            let target = BlockPos::new(
                catalyst.0.x + rng.random_range(-8i32..=8),
                catalyst.0.y + rng.random_range(-4i32..=4),
                catalyst.0.z + rng.random_range(-8i32..=8),
            );
            let (block, _) = world.get_block_and_state_id(&target);
            if !block.has_tag(&tag::Block::MINECRAFT_SCULK_REPLACEABLE) {
                continue;
            }
            let above_state = world.get_block_state(&target.up());
            if !(above_state.is_air() || above_state.is_liquid()) {
                continue;
            }
            let mut event = crate::plugin::api::events::block::sculk_bloom::SculkBloomEvent::new(
                target,
                world.clone(),
                charge,
            );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    continue;
                }
            }
            world.set_block_state(&target, state, BlockFlags::NOTIFY_ALL);
            return;
        }
    }

    /// 在死亡位置周围 8 格（切比雪夫距离）内寻找幽匿催化体，
    /// 对应原版 8 格半径的实体死亡游戏事件监听。
    pub fn find_nearby_catalyst(world: &Arc<World>, center: &BlockPos) -> Option<BlockPos> {
        for dx in -8..=8 {
            for dy in -8..=8 {
                for dz in -8..=8 {
                    let pos = BlockPos::new(center.0.x + dx, center.0.y + dy, center.0.z + dz);
                    if world.get_block(&pos) == &Block::SCULK_CATALYST {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }
}

impl BlockBehaviour for SculkCatalystBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = SculkCatalystLikeProperties::default(args.block);
        props.bloom = false;
        props.to_state_id(args.block)
    }
}
