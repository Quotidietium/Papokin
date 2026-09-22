//! 幽匿生长规则：在幽匿之上放置感测体 / 尖啸体。

use papokin_data::Block;
use papokin_data::BlockId;
use papokin_data::BlockState;
use papokin_data::block_properties::SculkSensorLikeProperties;
use papokin_data::block_properties::SculkShriekerLikeProperties;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_util::random::RandomGenerator;
use papokin_util::random::RandomImpl;

use super::SculkLevel;

/// 邻近生长物扫描的半径：XZ 方向 -4..4，Y 方向 0..2。
const GROWTH_CHECK_RADIUS: i32 = 4;
const GROWTH_CHECK_HEIGHT: i32 = 2;
/// 阻止放置前允许存在的附近生长物最大数量。
const MAX_NEARBY_GROWTHS: i32 = 2;

/// 幽匿感测体/尖啸体的放置规则。
pub struct GrowthRules;

impl GrowthRules {
    /// 检查生长物能否放置在 `pos` 上方：其上方的方块
    /// 必须是空气或水，且附近生长物必须稀少。
    pub fn can_place_growth(level: &dyn SculkLevel, pos: BlockPos) -> bool {
        let above = pos.up();
        let Some(above_state) = level.sculk_get(above) else {
            return false;
        };
        let above_id = above_state.to_block_id();
        // 上方方块必须是空气，或者是含水的水方块。
        if !(above_state.to_state().is_air()
            || (above_id == BlockId::WATER && level.sculk_is_water(above)))
        {
            return false;
        }
        // 统计附近的生长体（sensor + shrieker）。
        let mut growth_count = 0i32;
        for dx in -GROWTH_CHECK_RADIUS..=GROWTH_CHECK_RADIUS {
            for dz in -GROWTH_CHECK_RADIUS..=GROWTH_CHECK_RADIUS {
                for dy in 0..=GROWTH_CHECK_HEIGHT {
                    let check_pos = pos.offset(Vector3::new(dx, dy, dz));
                    if let Some(s) = level.sculk_get(check_pos) {
                        let id = s.to_block_id();
                        // 只有幽匿感测体和幽匿尖啸体计入
                        // （精确方块匹配——校准传感器则不匹配）。
                        if id == BlockId::SCULK_SENSOR || id == BlockId::SCULK_SHRIEKER {
                            growth_count += 1;
                            if growth_count > MAX_NEARBY_GROWTHS {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    ///返回幽匿感测体（10/11 概率）或幽匿尖啸体
    /// (1/11 概率)，放置在水中时为含水状态。
    pub fn random_growth_state(
        level: &dyn SculkLevel,
        pos: BlockPos,
        random: &mut RandomGenerator,
        is_world_gen: bool,
    ) -> &'static BlockState {
        let base_state = if random.next_bounded_i32(11) == 0 {
            // 尖啸体（1/11 概率）。
            let mut props = SculkShriekerLikeProperties::default(&Block::SCULK_SHRIEKER);
            props.r#can_summon = is_world_gen;
            BlockState::from_id(props.to_state_id(&Block::SCULK_SHRIEKER))
        } else {
            Block::SCULK_SENSOR.default_state
        };
        // 若该位置含水，则应用含水状态。
        if level.sculk_is_water(pos) {
            let block_id = base_state.id.to_block_id();
            if block_id == BlockId::SCULK_SHRIEKER {
                let mut props = SculkShriekerLikeProperties::from_state_id(base_state.id);
                props.r#waterlogged = true;
                return BlockState::from_id(props.to_state_id(&Block::SCULK_SHRIEKER));
            } else if block_id == BlockId::SCULK_SENSOR {
                let mut props = SculkSensorLikeProperties::from_state_id(base_state.id);
                props.r#waterlogged = true;
                return BlockState::from_id(props.to_state_id(&Block::SCULK_SENSOR));
            }
        }
        base_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::feature::features::sculk::test_utils::MockSculkLevel;
    use papokin_util::math::vector3::Vector3;

    #[test]
    fn shrieker_state_default() {
        let props = SculkShriekerLikeProperties::default(&Block::SCULK_SHRIEKER);
        assert!(!props.r#can_summon);
    }

    #[test]
    fn can_place_growth_accepts_air_like_states() {
        // 此处洞穴空气与虚空空气均视为空气，不仅限于普通的
        // `minecraft:air` 方块。
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos.up(), Block::CAVE_AIR.default_state.id);
        assert!(GrowthRules::can_place_growth(&level, pos));
    }

    #[test]
    fn can_place_growth_accepts_water_above() {
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos.up(), Block::WATER.default_state.id);
        assert!(GrowthRules::can_place_growth(&level, pos));
    }

    #[test]
    fn density_ignores_calibrated_sensors() {
        // 只有 SCULK_SENSOR / SCULK_SHRIEKER 计入；附近三个
        // 校频传感器不得阻止放置。
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos.up(), Block::AIR.default_state.id);
        for dx in 0..3 {
            level.set_id(
                pos.offset(Vector3::new(dx, 0, 0)),
                Block::CALIBRATED_SCULK_SENSOR.default_state.id,
            );
        }
        assert!(GrowthRules::can_place_growth(&level, pos));
    }

    #[test]
    fn density_blocks_after_two_sensors_or_shriekers() {
        let mut level = MockSculkLevel::new();
        let pos = BlockPos::new(0, 60, 0);
        level.set_id(pos.up(), Block::AIR.default_state.id);
        for dx in 0..2 {
            level.set_id(
                pos.offset(Vector3::new(dx, 0, 0)),
                Block::SCULK_SENSOR.default_state.id,
            );
        }
        assert!(GrowthRules::can_place_growth(&level, pos));
        level.set_id(
            pos.offset(Vector3::new(2, 0, 0)),
            Block::SCULK_SHRIEKER.default_state.id,
        );
        assert!(!GrowthRules::can_place_growth(&level, pos));
    }
}
