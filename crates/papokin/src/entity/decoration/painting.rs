use core::f32;
use std::sync::atomic::Ordering;

use crate::entity::{Entity, EntityBase, living::LivingEntity};
use papokin_data::BlockDirection;
use papokin_data::damage::DamageType;
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::vector3::Vector3;

/// 世界以水平值存储画的朝向：0 为南，1 为西，
/// 2 为北、3 为东（`Direction.get2DDataValue`）。实体数据以及生成
/// 由此构建的数据包，请改用三维索引：北 2、南 3、西 4、
/// 东 5。发送原始水平值会让客户端把 0 读作向下，
/// 在 `HangingEntity.setDirection` 的水平检查中失败并断开连接。
const fn facing_from_horizontal(value: u8) -> BlockDirection {
    match value & 3 {
        1 => BlockDirection::West,
        2 => BlockDirection::North,
        3 => BlockDirection::East,
        _ => BlockDirection::South,
    }
}

/// 原版用 `getByte` 读取该字段，当其为……时得到 0（南）
/// 缺失，因此不完整的实体文件会保留水平方向的默认值。
fn facing_from_nbt(nbt: &NbtCompound) -> BlockDirection {
    facing_from_horizontal(nbt.get_byte("facing").unwrap_or(0) as u8)
}

const fn facing_to_horizontal(direction: BlockDirection) -> u8 {
    match direction {
        BlockDirection::West => 1,
        BlockDirection::North => 2,
        BlockDirection::East => 3,
        _ => 0,
    }
}

pub struct PaintingEntity {
    entity: Entity,
}

impl PaintingEntity {
    pub const fn new(entity: Entity) -> Self {
        Self { entity }
    }

    /// 触发 `HangingBreakEvent`（当移除者是实体时还会触发 `HangingBreakByEntityEvent`）
    /// 移除者已知）并返回该画是否可能破碎。
    fn fire_break_events(&self, caused_by: Option<&dyn EntityBase>) -> bool {
        let world = self.entity.world.load();
        let Some(entity_arc) = world.get_entity_by_id(self.entity.entity_id) else {
            return true;
        };
        let remover = caused_by.and_then(|c| world.get_entity_by_id(c.get_entity().entity_id));

        let mut event = crate::plugin::api::events::hanging::hanging_break::HangingBreakEvent::new(
            entity_arc.clone(),
            remover.clone(),
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return false;
        }

        if let Some(remover) = remover {
            let mut by_entity_event = crate::plugin::api::events::hanging::hanging_break_by_entity::HangingBreakByEntityEvent::new(
                entity_arc, remover,
            );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_entity_event);
            }
            if by_entity_event.cancelled {
                return false;
            }
        }
        true
    }
}

impl EntityBase for PaintingEntity {
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let index = self.entity.data.load(Ordering::Relaxed) as u8;
        let direction = BlockDirection::from_index(index).unwrap_or(BlockDirection::South);
        nbt.put_byte("facing", facing_to_horizontal(direction) as i8);
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        let facing = facing_from_nbt(nbt);
        self.entity
            .data
            .store(i32::from(facing.to_index()), Ordering::Relaxed);
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        if !self.fire_break_events(source) {
            return true;
        }
        self.entity.remove();
        true
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_and_3d_facings_round_trip() {
        for horizontal in 0u8..4 {
            let direction = facing_from_horizontal(horizontal);
            assert!(direction.is_horizontal(), "{direction:?} is vertical");
            assert_eq!(facing_to_horizontal(direction), horizontal);
        }
    }

    #[test]
    fn south_two_is_not_down_zero() {
        // 旧 bug：南方（世界文件中的 0）被作为索引 0 发送，而这会被
        // 客户端会读作 down 并拒绝。
        assert_eq!(facing_from_horizontal(0), BlockDirection::South);
        assert_eq!(facing_from_horizontal(0).to_index(), 3);
    }

    #[test]
    fn missing_facing_defaults_to_south() {
        let nbt = NbtCompound::new();
        assert_eq!(facing_from_nbt(&nbt), BlockDirection::South);
        assert_eq!(facing_from_nbt(&nbt).to_index(), 3);

        let mut with_facing = NbtCompound::new();
        with_facing.put_byte("facing", 1);
        assert_eq!(facing_from_nbt(&with_facing), BlockDirection::West);
    }

    #[test]
    fn out_of_range_values_wrap_into_horizontal_directions() {
        assert!(facing_from_horizontal(4).is_horizontal());
        assert!(facing_from_horizontal(255).is_horizontal());
    }
}
