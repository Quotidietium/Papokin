use std::sync::Arc;
use std::sync::atomic::Ordering;

use crossbeam::atomic::AtomicCell;
use papokin_data::{Block, BlockDirection, BlockState};
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::world::{BlockFlags, World};

use super::BlockEntity;

pub struct PistonBlockEntity {
    pub position: BlockPos,
    pub pushed_block_state: &'static BlockState,
    pub facing: BlockDirection,
    pub current_progress: AtomicCell<f32>,
    pub last_progress: AtomicCell<f32>,
    pub extending: bool,
    pub source: bool,
}

impl PistonBlockEntity {
    pub const ID: &'static str = "minecraft:piston";

    const fn movement_direction(&self) -> BlockDirection {
        if self.extending {
            self.facing
        } else {
            self.facing.opposite()
        }
    }

    /// 原版的 `getAmountExtended`：距方块最终位置往回多远
    /// 视觉效果在给定动画进度下的伸展程度。负值表示伸展。
    fn amount_extended(&self, progress: f32) -> f32 {
        if self.extending {
            progress - 1.0
        } else {
            1.0 - progress
        }
    }

    fn dir_vec(dir: BlockDirection, scale: f64) -> Vector3<f64> {
        let off = dir.to_offset();
        Vector3::new(
            f64::from(off.x) * scale,
            f64::from(off.y) * scale,
            f64::from(off.z) * scale,
        )
    }

    /// 移植自原版 `PistonBlockEntity.pushEntities`：推动那些
    /// 边界框在本刻内与移动方块的扫掠体积相交。
    fn push_entities(&self, world: &Arc<World>, new_progress: f32) {
        let last = self.last_progress.load();
        let delta = f64::from(new_progress - last);
        if delta <= 0.0 {
            return;
        }

        let motion_dir = self.movement_direction();
        let amount = f64::from(self.amount_extended(last));

        // 当前视觉位置（本刻开始时）的方块 AABB。
        // 对于 source=true（活塞头方块实体），原版使用活塞头
        // 碰撞形状 —— 前表面上厚 4 像素的板 —— 而非
        // 整个完整方块。没有这条，头部会"扫过"过大的体积，
        // 回缩时会将实体（如末影水晶）一起拖回。
        let raw_block_aabb = BoundingBox::from_block(&self.position);
        let block_aabb = if self.source {
            Self::head_shape_aabb(raw_block_aabb, self.facing)
        } else {
            raw_block_aabb
        }
        .shift(Self::dir_vec(self.facing, amount));

        // 按运动量拉伸以获得扫掠体积（一刻的动画）。
        let motion = Self::dir_vec(motion_dir, delta);
        let swept = block_aabb.stretch(motion);

        for entity in world.get_entities_at_box(&swept) {
            let e = entity.get_entity();
            if e.no_physics.load(Ordering::Relaxed) {
                continue;
            }
            // 玩家移动由客户端权威；原版仍会对其进行微调
            // 通过 Entity.move(PISTON) 处理，但我们在此跳过它们以避免传送卡顿。
            if entity.get_player().is_some() {
                continue;
            }

            let entity_aabb = e.bounding_box.load();
            let intersection = Self::intersection_size(swept, motion_dir, entity_aabb);
            if intersection <= 0.0 {
                continue;
            }
            let push_amount = intersection.min(delta) + 0.01;
            Self::move_entity(e, motion_dir, push_amount);

            // 对于正在收回的活塞头，原版还会把实体推出
            // 活塞主体方块。没有这一步，实体会被拉入
            // 活塞并“粘”在其上（看起来像粘性活塞的拖拽）。
            // 对于 retract-head 方块实体，`self.position` 已经就是活塞
            // 方块位置（它在动画期间替换活塞）。
            if !self.extending && self.source {
                Self::push_out_of_piston_body(e, &self.position, motion_dir, delta);
            }
        }
    }

    /// 原版 `getIntersectionSize`：`entity` 沿……与 `swept` 重叠的程度
    /// `motion_dir`。正值表示实体处于移动方块的路径上。
    fn intersection_size(
        swept: BoundingBox,
        motion_dir: BlockDirection,
        entity: BoundingBox,
    ) -> f64 {
        match motion_dir {
            BlockDirection::East => swept.max.x - entity.min.x,
            BlockDirection::West => entity.max.x - swept.min.x,
            BlockDirection::Up => swept.max.y - entity.min.y,
            BlockDirection::Down => entity.max.y - swept.min.y,
            BlockDirection::South => swept.max.z - entity.min.z,
            BlockDirection::North => entity.max.z - swept.min.z,
        }
    }

    fn move_entity(entity: &crate::entity::Entity, dir: BlockDirection, distance: f64) {
        let new_pos = entity.pos.load() + Self::dir_vec(dir, distance);
        entity.set_pos(new_pos);
        entity.send_pos();
    }

    /// 原版 `push`：当活塞头缩回时，推开最终落在……内的实体
    /// 位于活塞主体立方体内的物体沿相反方向推出（略微超过
    /// 刚刚获得的移动，因此净位移几乎为零）。
    fn push_out_of_piston_body(
        entity: &crate::entity::Entity,
        piston_pos: &BlockPos,
        motion_dir: BlockDirection,
        amount: f64,
    ) {
        let body_aabb = BoundingBox::from_block(piston_pos);
        let entity_aabb = entity.bounding_box.load();
        if !body_aabb.intersects(&entity_aabb) {
            return;
        }
        let back = motion_dir.opposite();
        let e = Self::intersection_size(body_aabb, back, entity_aabb) + 0.01;
        let f = Self::intersection_size(
            body_aabb,
            back,
            Self::aabb_intersection(body_aabb, entity_aabb),
        ) + 0.01;
        if (e - f).abs() < 0.01 {
            let distance = e.min(amount) + 0.01;
            Self::move_entity(entity, back, distance);
        }
    }

    /// 近似原版 `PistonHeadBlock` 的碰撞形状：一个 4 像素（厚）的方块，
    /// (0.25 格) 台阶，位于方块沿朝向的前表面。
    fn head_shape_aabb(block_aabb: BoundingBox, facing: BlockDirection) -> BoundingBox {
        const HEAD_THICKNESS: f64 = 0.25;
        let mut min = block_aabb.min;
        let mut max = block_aabb.max;
        match facing {
            BlockDirection::East => min.x = max.x - HEAD_THICKNESS,
            BlockDirection::West => max.x = min.x + HEAD_THICKNESS,
            BlockDirection::Up => min.y = max.y - HEAD_THICKNESS,
            BlockDirection::Down => max.y = min.y + HEAD_THICKNESS,
            BlockDirection::South => min.z = max.z - HEAD_THICKNESS,
            BlockDirection::North => max.z = min.z + HEAD_THICKNESS,
        }
        BoundingBox::new(min, max)
    }

    const fn aabb_intersection(a: BoundingBox, b: BoundingBox) -> BoundingBox {
        BoundingBox::new(
            Vector3::new(
                a.min.x.max(b.min.x),
                a.min.y.max(b.min.y),
                a.min.z.max(b.min.z),
            ),
            Vector3::new(
                a.max.x.min(b.max.x),
                a.max.y.min(b.max.y),
                a.max.z.min(b.max.z),
            ),
        )
    }

    pub fn finish(&self, world: &Arc<World>) {
        if self.last_progress.load() < 1.0 {
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                let state = if self.source {
                    Block::AIR.default_state.id
                } else {
                    world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos)
                };
                world.set_block_state(&pos, state, BlockFlags::NOTIFY_ALL);
                world.update_neighbors(&pos, None);
            }
        }
    }
}

const FACING: &str = "facing";
const LAST_PROGRESS: &str = "progress";
const EXTENDING: &str = "extending";
const SOURCE: &str = "source";
const BLOCK_STATE: &str = "blockState";

impl BlockEntity for PistonBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn tick(&self, world: &Arc<World>) {
        let current_progress = self.current_progress.load();
        self.last_progress.store(current_progress);
        if current_progress >= 1.0 {
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                if self.pushed_block_state.is_air() {
                    // 必须通知客户端，否则 moving_piston 会残留在客户端
                    // 永远隐形（客户端不渲染没有方块实体的 moving_piston）
                    world.set_block_state(
                        &pos,
                        self.pushed_block_state.id,
                        BlockFlags::NOTIFY_ALL | BlockFlags::FORCE_STATE | BlockFlags::MOVED,
                    );
                } else {
                    let updated_state =
                        world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos);
                    world.set_block_state(
                        &pos,
                        updated_state,
                        BlockFlags::NOTIFY_ALL | BlockFlags::MOVED,
                    );
                    world.update_neighbors(&pos, None);
                }
            }
            return;
        }
        let new_progress = (current_progress + 0.5).min(1.0);
        self.push_entities(world, new_progress);
        self.current_progress.store(new_progress);
    }

    fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        // 不带命名空间的老存档/残缺数据一律回退为空气
        let pushed_block_state = nbt
            .get_string(BLOCK_STATE)
            .and_then(BlockState::from_state_string)
            .unwrap_or(Block::AIR.default_state);
        let facing = nbt.get_byte(FACING).unwrap_or(0);
        let last_progress = nbt.get_float(LAST_PROGRESS).unwrap_or(0.0);
        let extending = nbt.get_bool(EXTENDING).unwrap_or(false);
        let source = nbt.get_bool(SOURCE).unwrap_or(false);
        Self {
            pushed_block_state,
            position,
            facing: BlockDirection::from_index(facing as u8).unwrap_or(BlockDirection::Down),
            current_progress: last_progress.into(),
            last_progress: last_progress.into(),
            extending,
            source,
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string(BLOCK_STATE, self.pushed_block_state.to_state_string());
        nbt.put_byte(FACING, self.facing.to_index() as i8);
        nbt.put_float(LAST_PROGRESS, self.last_progress.load());
        nbt.put_bool(EXTENDING, self.extending);
        nbt.put_bool(SOURCE, self.source);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        // 客户端的移动动画与最终落块都依赖 blockState；缺失时客户端
        // 把被移动方块当作空气渲染，动画结束还会把目标位置覆盖成空气
        nbt.put_string(BLOCK_STATE, self.pushed_block_state.to_state_string());
        nbt.put_byte(FACING, self.facing.to_index() as i8);
        nbt.put_float(LAST_PROGRESS, self.last_progress.load());
        nbt.put_bool(EXTENDING, self.extending);
        nbt.put_bool(SOURCE, self.source);
        // TODO: 由于异步导致代码重复 :c
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::PistonBlockEntity;
    use crate::block::entities::BlockEntity;
    use papokin_data::{Block, BlockDirection};
    use papokin_nbt::compound::NbtCompound;
    use papokin_util::math::position::BlockPos;

    /// `blockState` 字段是客户端渲染移动方块、动画结束落块的依据；
    /// 此前该字段缺失（恒为空气），粘性活塞推动的方块会在客户端
    /// 消失不显示。序列化与反序列化都必须保真。
    #[test]
    fn piston_entity_nbt_preserves_pushed_block_state() {
        let entity = PistonBlockEntity {
            position: BlockPos::new(1, 64, 2),
            pushed_block_state: Block::REDSTONE_BLOCK.default_state,
            facing: BlockDirection::North,
            current_progress: 0.5.into(),
            last_progress: 0.5.into(),
            extending: true,
            source: false,
        };

        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt);
        assert_eq!(
            nbt.get_string("blockState"),
            Some("minecraft:redstone_block"),
            "blockState 字段缺失会导致客户端把方块当作空气"
        );

        let restored = PistonBlockEntity::from_nbt(&nbt, BlockPos::new(1, 64, 2));
        assert_eq!(
            restored.pushed_block_state,
            Block::REDSTONE_BLOCK.default_state
        );
        assert_eq!(restored.facing, BlockDirection::North);
        assert!(restored.extending);
    }

    /// 带属性的方块（如粘性活塞头）也必须完整往返。
    #[test]
    fn piston_entity_nbt_round_trips_property_heavy_states() {
        for block in [
            Block::STICKY_PISTON,
            Block::OAK_STAIRS,
            Block::CHORUS_FLOWER,
        ] {
            let entity = PistonBlockEntity {
                position: BlockPos::new(0, 64, 0),
                pushed_block_state: block.default_state,
                facing: BlockDirection::Up,
                current_progress: 0.0.into(),
                last_progress: 0.0.into(),
                extending: false,
                source: true,
            };
            let mut nbt = NbtCompound::new();
            entity.write_nbt(&mut nbt);
            let restored = PistonBlockEntity::from_nbt(&nbt, BlockPos::new(0, 64, 0));
            assert_eq!(
                restored.pushed_block_state, block.default_state,
                "{} 的状态经 NBT 往返后改变",
                block.name
            );
        }
    }
}
