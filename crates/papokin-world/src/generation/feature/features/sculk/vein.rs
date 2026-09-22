//! 幽匿脉络沿表面蔓延（多面生长）。

use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::BlockId;
use papokin_data::BlockState;
use papokin_data::BlockStateId;
use papokin_data::block_properties::GlowLichenLikeProperties;
use papokin_util::math::position::BlockPos;
use papokin_util::random::RandomGenerator;
use papokin_util::random::RandomImpl;

use super::SculkLevel;
use super::is_sculk_replaceable;

/// 三个多面（multiface）展开位置。
#[derive(Debug, Clone, Copy)]
pub enum SpreadType {
    /// 放置在同一位置，朝向 `spread_direction`。
    SamePosition,
    /// 放置在 `spread_direction` 方向的相邻位置，朝向 `from_face`。
    SamePlane,
    /// 环绕处理：放置在邻位 + `from_face` 处，朝向相反。
    WrapAround,
}

impl SpreadType {
    /// 计算新幽匿脉络的目标位置及其应具有的面。
    #[must_use]
    pub fn spread_pos(
        self,
        pos: BlockPos,
        spread_direction: BlockDirection,
        from_face: BlockDirection,
    ) -> (BlockPos, BlockDirection) {
        match self {
            Self::SamePosition => (pos, spread_direction),
            Self::SamePlane => (pos.offset(spread_direction.to_offset()), from_face),
            Self::WrapAround => (
                pos.offset(spread_direction.to_offset())
                    .offset(from_face.to_offset()),
                spread_direction.opposite(),
            ),
        }
    }
}

/// 尝试扩散位置的顺序。
pub const DEFAULT_SPREAD_ORDER: [SpreadType; 3] = [
    SpreadType::SamePosition,
    SpreadType::SamePlane,
    SpreadType::WrapAround,
];

/// 幽匿脉络在方块各表面蔓延的规则。
pub struct VeinRules;

impl VeinRules {
    /// 尝试在与 `pos` 相邻的支撑方块上放置幽匿块。
    pub fn attempt_place_sculk(
        level: &mut dyn SculkLevel,
        pos: BlockPos,
        random: &mut RandomGenerator,
        replaceable: impl Fn(BlockId) -> bool,
    ) -> bool {
        let Some(state) = level.sculk_get(pos) else {
            return false;
        };

        // 按打乱后的顺序尝试各支撑方向。
        let mut support_order = BlockDirection::all();
        for i in (1..support_order.len()).rev() {
            let j = random.next_bounded_i32((i + 1) as i32) as usize;
            support_order.swap(i, j);
        }

        for support in support_order {
            if !Self::has_face(state, support) {
                continue;
            }
            let support_pos = pos.offset(support.to_offset());
            let Some(support_state) = level.sculk_get(support_pos) else {
                continue;
            };
            let support_id = support_state.to_block_id();
            if !replaceable(support_id) {
                continue;
            }
            // 在支撑位置放置幽匿块。
            level.sculk_set(support_pos, Block::SCULK.default_state);
            // 先从新的 Sculk 方块蔓延脉络，然后释放
            // 周围的矿脉。
            Self::spread_all(level, support_pos);
            // 然后对周围的矿脉放电。
            let skip = support.opposite();
            for vein_dir in BlockDirection::all() {
                if vein_dir == skip {
                    continue;
                }
                let vein_pos = support_pos.offset(vein_dir.to_offset());
                if let Some(vs) = level.sculk_get(vein_pos)
                    && vs.to_block_id() == BlockId::SCULK_VEIN
                {
                    Self::on_discharged(level, vein_pos);
                }
            }
            return true;
        }
        false
    }

    /// 光标位置的默认矿脉扩散行为。
    pub fn attempt_spread_vein(
        level: &mut dyn SculkLevel,
        pos: BlockPos,
        state: Option<BlockStateId>,
        faces: Option<u8>,
    ) -> bool {
        match faces {
            // 未指定朝向面时，只适用同位置的传播。
            None => Self::spread_same_space(level, pos),
            // 面集合为空时，按完整的多面传播顺序
            // 适用。
            Some(0) => Self::spread_all(level, pos),
            // 已设置面时重新生长矿脉，前提是该方块为
            // 空气或含水。
            Some(faces_bits) => {
                let Some(existing) = state.or_else(|| level.sculk_get(pos)) else {
                    return false;
                };
                if !Self::state_is_air_or_water(level, existing, pos) {
                    return false;
                }
                let faces: Vec<BlockDirection> = BlockDirection::all()
                    .into_iter()
                    .filter(|dir| faces_bits & (1 << dir.to_index()) != 0)
                    .collect();
                Self::regrow(level, pos, &faces)
            }
        }
    }

    /// 重新长出可在 `pos` 处附着的矿脉面。
    pub fn regrow(level: &mut dyn SculkLevel, pos: BlockPos, faces: &[BlockDirection]) -> bool {
        let mut has_any = false;
        // 始终从全新的 sculk_vein 默认状态开始，
        // 丢弃上一个状态遗留的过时朝向属性。
        let mut new_state = Block::SCULK_VEIN.default_state.id;
        for face in faces {
            if Self::can_attach_to(level, pos, *face) {
                new_state = Self::with_face(new_state, *face, true);
                has_any = true;
            }
        }
        if !has_any {
            return false;
        }
        // 保留现有方块流体状态中的含水状态。
        if let Some(existing) = level.sculk_get(pos)
            && Self::state_has_water(level, existing, pos)
        {
            new_state = Self::with_waterlogged(new_state, true);
        }
        level.sculk_set(pos, new_state.to_state());
        true
    }

    /// 检查幽匿脉络是否有可替换的基质可供生长。
    pub fn has_substrate_access(
        level: &dyn SculkLevel,
        state: BlockStateId,
        pos: BlockPos,
    ) -> bool {
        if state.to_block_id() != BlockId::SCULK_VEIN {
            return false;
        }
        BlockDirection::all().into_iter().any(|dir| {
            if !Self::has_face(state, dir) {
                return false;
            }
            let neighbour = pos.offset(dir.to_offset());
            level
                .sculk_get(neighbour)
                .is_some_and(|s| is_sculk_replaceable(s.to_block_id()))
        })
    }

    /// 在电荷耗尽时，移除失去支撑的幽匿脉络面。
    pub fn on_discharged(level: &mut dyn SculkLevel, pos: BlockPos) {
        let Some(state) = level.sculk_get(pos) else {
            return;
        };
        if state.to_block_id() != BlockId::SCULK_VEIN {
            return;
        }
        let mut new_state = state;
        // 移除邻域变为幽匿脉络的面，保留
        // 附着于非幽匿（sculk）支撑体上。
        for dir in BlockDirection::all() {
            if Self::has_face(new_state, dir) {
                let neighbour = pos.offset(dir.to_offset());
                let is_sculk = level
                    .sculk_get(neighbour)
                    .is_some_and(|ns| ns.to_block_id() == BlockId::SCULK);
                if is_sculk {
                    new_state = Self::with_face(new_state, dir, false);
                }
            }
        }
        // 如果没有剩余面，则替换为空气（或水）。
        if !Self::has_any_face(new_state) {
            new_state = if level.sculk_is_water(pos) {
                Block::WATER.default_state.id
            } else {
                Block::AIR.default_state.id
            };
        }
        level.sculk_set(pos, new_state.to_state());
    }

    /// 从源的所有面传播。
    pub fn spread_all(level: &mut dyn SculkLevel, pos: BlockPos) -> bool {
        Self::spread_with_types(level, pos, &DEFAULT_SPREAD_ORDER)
    }

    /// 仅限同位置蔓延。
    fn spread_same_space(level: &mut dyn SculkLevel, pos: BlockPos) -> bool {
        Self::spread_with_types(level, pos, &[SpreadType::SamePosition])
    }

    fn spread_with_types(
        level: &mut dyn SculkLevel,
        pos: BlockPos,
        spread_types: &[SpreadType],
    ) -> bool {
        // 源状态只捕获一次，并在每个面之间复用：
        // 调用期间在 `pos` 处添加的面不得成为新的
        // 同一次调用内的多个水源。
        let Some(source_state) = level.sculk_get(pos) else {
            return false;
        };
        let mut any = false;
        for face in BlockDirection::all() {
            if Self::can_spread_from_face(source_state, face)
                && Self::spread_from_face(level, pos, source_state, face, spread_types)
            {
                any = true;
            }
        }
        any
    }

    fn can_spread_from_face(state: BlockStateId, face: BlockDirection) -> bool {
        let id = state.to_block_id();
        // 必须具有该面或是非矿脉方块。
        if id == BlockId::SCULK_VEIN {
            Self::has_face(state, face)
        } else {
            true
        }
    }

    fn spread_from_face(
        level: &mut dyn SculkLevel,
        pos: BlockPos,
        source_state: BlockStateId,
        from_face: BlockDirection,
        spread_types: &[SpreadType],
    ) -> bool {
        // 在外层循环中迭代方向，在内层迭代扩散类型
        // 内层循环，每个（面，方向）组合最多放置一条矿脉。
        let source_id = source_state.to_block_id();
        let is_vein = source_id == BlockId::SCULK_VEIN;
        let mut any = false;

        for spread_dir in BlockDirection::all() {
            if spread_dir.to_axis() == from_face.to_axis() {
                continue;
            }
            // 对于幽匿脉络（sculk vein）源，蔓延方向不得
            // 已设置了 face（朝向）。
            if is_vein && Self::has_face(source_state, spread_dir) {
                continue;
            }
            for spread_type in spread_types {
                let (target_pos, target_face) = spread_type.spread_pos(pos, spread_dir, from_face);
                if Self::can_spread_into(level, pos, target_pos, target_face) {
                    let old_state = level.sculk_get(target_pos);
                    if let Some(placed) =
                        Self::get_state_for_placement(level, target_pos, target_face, old_state)
                    {
                        level.sculk_set(target_pos, placed);
                        any = true;
                        break;
                    }
                }
            }
        }
        any
    }

    /// 检查幽匿脉络能否蔓延到目标位置。
    fn can_spread_into(
        level: &dyn SculkLevel,
        source_pos: BlockPos,
        placement_pos: BlockPos,
        placement_face: BlockDirection,
    ) -> bool {
        let Some(existing) = level.sculk_get(placement_pos) else {
            return false;
        };
        let existing_id = existing.to_block_id();
        // 当放置面背后的支撑方块
        // 幽匿、sculk_catalyst 或移动中的活塞。
        let against_pos = placement_pos.offset(placement_face.to_offset());
        if let Some(against) = level.sculk_get(against_pos) {
            let against_id = against.to_block_id();
            if against_id == BlockId::SCULK
                || against_id == BlockId::SCULK_CATALYST
                || against_id == BlockId::MOVING_PISTON
            {
                return false;
            }
        }
        // 曼哈顿距离 2 检查。
        let manhattan = (placement_pos.0.x - source_pos.0.x).abs()
            + (placement_pos.0.y - source_pos.0.y).abs()
            + (placement_pos.0.z - source_pos.0.z).abs();
        if manhattan == 2 {
            let neighbor_pos = source_pos.offset(placement_face.opposite().to_offset());
            if level.sculk_is_face_sturdy(neighbor_pos, placement_face) {
                return false;
            }
        }
        // 火焰检查
        if existing_id == Block::FIRE.id {
            return false;
        }
        // 非水流体无法被替换：现有流体状态
        // 必须为空或水。
        if existing.to_state().is_liquid()
            && !(existing_id == BlockId::WATER && level.sculk_is_water(placement_pos))
        {
            return false;
        }
        // 接受可替换状态、空气、同种脉络方块，或
        // 水源。
        existing.to_state().replaceable()
            || existing_id == BlockId::AIR
            || existing_id == BlockId::SCULK_VEIN
            || (existing_id == BlockId::WATER && level.sculk_is_water_source(placement_pos))
    }

    fn get_state_for_placement(
        level: &dyn SculkLevel,
        placement_pos: BlockPos,
        face: BlockDirection,
        old_state: Option<BlockStateId>,
    ) -> Option<&'static BlockState> {
        // 放置需要面后方有坚固支撑，且该面
        // 必须尚未被设置。
        if !Self::can_attach_to(level, placement_pos, face) {
            return None;
        }
        if let Some(old) = old_state
            && old.to_block_id() == BlockId::SCULK_VEIN
            && Self::has_face(old, face)
        {
            return None;
        }
        // 确定基础状态：若已是 sculk_vein，则延伸它；
        // 否则从全新的 sculk_vein 默认状态开始。
        let mut base = old_state.map_or(Block::SCULK_VEIN.default_state.id, |s| {
            if s.to_block_id() == BlockId::SCULK_VEIN {
                s
            } else {
                Block::SCULK_VEIN.default_state.id
            }
        });
        base = Self::with_face(base, face, true);
        // 先前状态含有水时保留含水状态
        // 水源。
        if let Some(s) = old_state
            && Self::old_state_is_water_source(level, s, placement_pos)
        {
            base = Self::with_waterlogged(base, true);
        }
        Some(base.to_state())
    }

    /// 检查幽匿脉络面能否附着于此：所对方块的
    /// 支撑方块必须坚固。
    fn can_attach_to(level: &dyn SculkLevel, pos: BlockPos, face: BlockDirection) -> bool {
        let support_pos = pos.offset(face.to_offset());
        level.sculk_is_face_sturdy(support_pos, face.opposite())
    }

    /// 检查给定状态是否含有水流体。
    fn state_has_water(level: &dyn SculkLevel, state: BlockStateId, pos: BlockPos) -> bool {
        match state.to_block_id() {
            BlockId::WATER => level.sculk_is_water(pos),
            BlockId::SCULK_VEIN => {
                let props = GlowLichenLikeProperties::from_state_id(state);
                props.r#waterlogged
            }
            _ => false,
        }
    }

    /// 重新生长前的门槛检查：状态必须为空气或含水。
    fn state_is_air_or_water(level: &dyn SculkLevel, state: BlockStateId, pos: BlockPos) -> bool {
        state.to_state().is_air() || Self::state_has_water(level, state, pos)
    }

    /// 检查之前的状态是否持有水源。
    fn old_state_is_water_source(
        level: &dyn SculkLevel,
        state: BlockStateId,
        pos: BlockPos,
    ) -> bool {
        match state.to_block_id() {
            BlockId::WATER => level.sculk_is_water_source(pos),
            BlockId::SCULK_VEIN => {
                let props = GlowLichenLikeProperties::from_state_id(state);
                props.r#waterlogged
            }
            _ => false,
        }
    }

    /// 返回 `state` 是否设置了给定的面标志位。
    #[must_use]
    pub fn has_face(state: BlockStateId, face: BlockDirection) -> bool {
        if state.to_block_id() != BlockId::SCULK_VEIN {
            return false;
        }
        let props = GlowLichenLikeProperties::from_state_id(state);
        match face {
            BlockDirection::Down => props.r#down,
            BlockDirection::Up => props.r#up,
            BlockDirection::North => props.r#north,
            BlockDirection::South => props.r#south,
            BlockDirection::West => props.r#west,
            BlockDirection::East => props.r#east,
        }
    }

    /// 返回是否设置了任意面标志位。
    #[must_use]
    pub fn has_any_face(state: BlockStateId) -> bool {
        BlockDirection::all()
            .into_iter()
            .any(|dir| Self::has_face(state, dir))
    }

    ///返回一个将给定面设置为 `value` 的新状态。
    #[must_use]
    pub fn with_face(state: BlockStateId, face: BlockDirection, value: bool) -> BlockStateId {
        let mut props = GlowLichenLikeProperties::from_state_id(state);
        match face {
            BlockDirection::Down => props.r#down = value,
            BlockDirection::Up => props.r#up = value,
            BlockDirection::North => props.r#north = value,
            BlockDirection::South => props.r#south = value,
            BlockDirection::West => props.r#west = value,
            BlockDirection::East => props.r#east = value,
        }
        BlockState::from_id(props.to_state_id(&Block::SCULK_VEIN)).id
    }

    fn with_waterlogged(state: BlockStateId, value: bool) -> BlockStateId {
        let mut props = GlowLichenLikeProperties::from_state_id(state);
        props.r#waterlogged = value;
        BlockState::from_id(props.to_state_id(&Block::SCULK_VEIN)).id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::feature::features::sculk::test_utils::MockSculkLevel;
    use papokin_util::math::vector3::Vector3;

    #[test]
    fn spread_all_ignores_faces_added_during_call() {
        // 源状态已做快照：北面被放置在下方
        // 不得在同一次调用中作为向上蔓延的来源。
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        let base = VeinRules::with_face(
            Block::SCULK_VEIN.default_state.id,
            BlockDirection::Down,
            true,
        );
        level.set_id(pos, base);
        level.set_id(
            pos.offset(BlockDirection::Down.to_offset()),
            Block::STONE.default_state.id,
        );
        level.set_id(
            pos.offset(BlockDirection::North.to_offset()),
            Block::STONE.default_state.id,
        );
        level.set_id(
            pos.offset(BlockDirection::Up.to_offset()),
            Block::STONE.default_state.id,
        );
        assert!(VeinRules::spread_all(&mut level, pos));
        let result = level.sculk_get(pos).expect("矿脉应保留");
        assert!(VeinRules::has_face(result, BlockDirection::Down));
        assert!(VeinRules::has_face(result, BlockDirection::North));
        assert!(!VeinRules::has_face(result, BlockDirection::Up));
    }

    #[test]
    fn discharged_waterlogged_vein_restores_water_in_proto_view() {
        // 无暴露面的水浸矿脉含水流体，因此排放
        // 必须恢复为水而不是空气。
        use crate::generation::feature::features::sculk::ProtoChunkSculkView;
        use crate::generation::get_world_gen;
        use crate::generation::proto_chunk::ProtoChunk;
        use papokin_data::dimension::Dimension;
        use papokin_util::world_seed::Seed;

        let world_gen = get_world_gen(
            Seed(1),
            Dimension::OVERWORLD,
            false,
            Vec::new(),
            String::new(),
        );
        let mut chunk = ProtoChunk::new(0, 0, &world_gen);
        let mut view = ProtoChunkSculkView::new(&mut chunk);
        let pos = BlockPos::new(0, 60, 0);
        let waterlogged = VeinRules::with_waterlogged(Block::SCULK_VEIN.default_state.id, true);
        view.sculk_set(pos, BlockState::from_id(waterlogged));
        VeinRules::on_discharged(&mut view, pos);
        assert!(
            view.sculk_get(pos)
                .is_some_and(|s| s.to_block_id() == BlockId::WATER)
        );
    }

    #[test]
    fn spread_type_same_position() {
        let pos = BlockPos::new(10, 60, 10);
        let (p, f) =
            SpreadType::SamePosition.spread_pos(pos, BlockDirection::Up, BlockDirection::North);
        assert_eq!(p, pos);
        assert_eq!(f, BlockDirection::Up);
    }

    #[test]
    fn spread_type_same_plane() {
        let pos = BlockPos::new(10, 60, 10);
        let (p, f) =
            SpreadType::SamePlane.spread_pos(pos, BlockDirection::East, BlockDirection::North);
        assert_eq!(p.0, Vector3::new(11, 60, 10));
        assert_eq!(f, BlockDirection::North);
    }

    #[test]
    fn spread_type_wrap_around() {
        let pos = BlockPos::new(10, 60, 10);
        let (p, f) =
            SpreadType::WrapAround.spread_pos(pos, BlockDirection::East, BlockDirection::North);
        assert_eq!(p.0, Vector3::new(11, 60, 9));
        assert_eq!(f, BlockDirection::West);
    }

    #[test]
    fn has_face_bit_operations() {
        let base = Block::SCULK_VEIN.default_state.id;
        assert!(!VeinRules::has_any_face(base));
        let with_up = VeinRules::with_face(base, BlockDirection::Up, true);
        assert!(VeinRules::has_face(with_up, BlockDirection::Up));
        assert!(!VeinRules::has_face(with_up, BlockDirection::Down));
        let removed = VeinRules::with_face(with_up, BlockDirection::Up, false);
        assert!(!VeinRules::has_any_face(removed));
    }
}
