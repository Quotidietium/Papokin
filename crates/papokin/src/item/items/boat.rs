use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::vehicle::boat::BoatEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::Block;
use papokin_data::entity::EntityType;
use papokin_data::fluid::Fluid;
use papokin_data::item::Item;
use papokin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

use crate::world::World;

pub struct BoatItem;

impl BoatItem {
    /// 将船物品映射到对应的实体类型
    pub(crate) fn item_to_entity(item: &Item) -> &'static EntityType {
        match item.id {
            val if val == Item::OAK_BOAT.id => &EntityType::OAK_BOAT,
            val if val == Item::OAK_CHEST_BOAT.id => &EntityType::OAK_CHEST_BOAT,
            val if val == Item::SPRUCE_BOAT.id => &EntityType::SPRUCE_BOAT,
            val if val == Item::SPRUCE_CHEST_BOAT.id => &EntityType::SPRUCE_CHEST_BOAT,
            val if val == Item::BIRCH_BOAT.id => &EntityType::BIRCH_BOAT,
            val if val == Item::BIRCH_CHEST_BOAT.id => &EntityType::BIRCH_CHEST_BOAT,
            val if val == Item::JUNGLE_BOAT.id => &EntityType::JUNGLE_BOAT,
            val if val == Item::JUNGLE_CHEST_BOAT.id => &EntityType::JUNGLE_CHEST_BOAT,
            val if val == Item::ACACIA_BOAT.id => &EntityType::ACACIA_BOAT,
            val if val == Item::ACACIA_CHEST_BOAT.id => &EntityType::ACACIA_CHEST_BOAT,
            val if val == Item::DARK_OAK_BOAT.id => &EntityType::DARK_OAK_BOAT,
            val if val == Item::DARK_OAK_CHEST_BOAT.id => &EntityType::DARK_OAK_CHEST_BOAT,
            val if val == Item::MANGROVE_BOAT.id => &EntityType::MANGROVE_BOAT,
            val if val == Item::MANGROVE_CHEST_BOAT.id => &EntityType::MANGROVE_CHEST_BOAT,
            val if val == Item::CHERRY_BOAT.id => &EntityType::CHERRY_BOAT,
            val if val == Item::CHERRY_CHEST_BOAT.id => &EntityType::CHERRY_CHEST_BOAT,
            val if val == Item::PALE_OAK_BOAT.id => &EntityType::PALE_OAK_BOAT,
            val if val == Item::PALE_OAK_CHEST_BOAT.id => &EntityType::PALE_OAK_CHEST_BOAT,
            val if val == Item::POPLAR_BOAT.id => &EntityType::POPLAR_BOAT,
            val if val == Item::POPLAR_CHEST_BOAT.id => &EntityType::POPLAR_CHEST_BOAT,
            val if val == Item::BAMBOO_RAFT.id => &EntityType::BAMBOO_RAFT,
            val if val == Item::BAMBOO_CHEST_RAFT.id => &EntityType::BAMBOO_CHEST_RAFT,
            _ => {
                tracing::error!("未知的船物品 ID：{}", item.id);
                &EntityType::OAK_BOAT
            }
        }
    }

    /// 获取该船类型的实体尺寸
    const fn get_entity_dimensions(entity_type: &EntityType) -> EntityDimensions {
        EntityDimensions::new(
            entity_type.dimension[0],
            entity_type.dimension[1],
            entity_type.eye_height,
        )
    }
}

impl ItemMetadata for BoatItem {
    fn ids() -> Box<[u16]> {
        [
            Item::OAK_BOAT.id,
            Item::OAK_CHEST_BOAT.id,
            Item::SPRUCE_BOAT.id,
            Item::SPRUCE_CHEST_BOAT.id,
            Item::BIRCH_BOAT.id,
            Item::BIRCH_CHEST_BOAT.id,
            Item::JUNGLE_BOAT.id,
            Item::JUNGLE_CHEST_BOAT.id,
            Item::ACACIA_BOAT.id,
            Item::ACACIA_CHEST_BOAT.id,
            Item::DARK_OAK_BOAT.id,
            Item::DARK_OAK_CHEST_BOAT.id,
            Item::MANGROVE_BOAT.id,
            Item::MANGROVE_CHEST_BOAT.id,
            Item::CHERRY_BOAT.id,
            Item::CHERRY_CHEST_BOAT.id,
            Item::PALE_OAK_BOAT.id,
            Item::PALE_OAK_CHEST_BOAT.id,
            Item::POPLAR_BOAT.id,
            Item::POPLAR_CHEST_BOAT.id,
            Item::BAMBOO_RAFT.id,
            Item::BAMBOO_CHEST_RAFT.id,
        ]
        .into()
    }
}

impl ItemBehaviour for BoatItem {
    /// 原版：`BoatItem.use()` - 射线检测以找到放置位置
    fn normal_use(&self, item: &Item, player: &Player) {
        let world = player.world();
        let (start_pos, end_pos) = self.get_start_and_end_pos(player);

        // 原版：使用 FluidHandling.ANY 进行射线检测——在水/熔岩表面或固体方块处停止
        let checker = |pos: &BlockPos, world_inner: &Arc<World>| {
            let state_id = world_inner.get_block_state_id(pos);

            // 空气不会阻挡射线检测
            if state_id == Block::AIR.default_state.id {
                return false;
            }

            // 检查是否为流体——在任何流体处停止（原版中为 FluidHandling.ANY）
            if Fluid::from_state_id(state_id).is_some() {
                return true;
            }

            // 遇到固体方块时停止
            true
        };

        let Some((hit_pos, _direction)) = world.raycast(start_pos, end_pos, checker) else {
            return;
        };

        // 计算命中位置——方块顶面中心或水面
        // TODO: 原版使用精确的射线检测交点（hitResult.getPos()），
        // Pumpkin 的射线检测只返回方块位置。
        let hit_vec = Vector3::new(
            f64::from(hit_pos.0.x) + 0.5,
            f64::from(hit_pos.0.y) + 1.0,
            f64::from(hit_pos.0.z) + 0.5,
        );

        // 原版：检查路径中是否有会阻碍放置的实体
        // 获取玩家旋转向量，按 5.0 拉伸并扩展 1.0
        let (yaw, pitch) = player.rotation();
        let rotation_vec =
            Vector3::rotation_vector(f64::from(pitch), f64::from(yaw)).multiply(5.0, 5.0, 5.0);
        let player_entity = player.get_entity().bounding_box.load();
        let search_box = player_entity.stretch(rotation_vec).expand_all(1.0);

        let entities = world.get_entities_at_box(&search_box);
        let player_eye_pos = player.eye_position();

        for entity in &entities {
            // 原版：EntityPredicates.CAN_HIT = !isSpectator() && canHit()
            if entity.is_spectator() || !entity.can_hit() {
                continue;
            }

            // 原版：按 getTargetingMargin() 扩展（多数实体为 0.0）
            let entity_box = entity.get_entity().bounding_box.load();
            // 检查实体包围盒是否包含玩家的眼睛位置
            if entity_box.intersects(&BoundingBox::new(player_eye_pos, player_eye_pos)) {
                // 实体在眼部位置挡住了视线
                return;
            }
        }

        // 创建船实体
        let entity_type = Self::item_to_entity(item);
        let dimensions = Self::get_entity_dimensions(entity_type);
        let boat_box = BoundingBox::new_from_pos(hit_vec.x, hit_vec.y, hit_vec.z, &dimensions);

        // 原版：if (!world.isSpaceEmpty(lv7, lv7.getBoundingBox())) return FAIL
        if !world.is_space_empty(boat_box) {
            return;
        }

        // 检查没有实体占据该空间
        if !world.get_entities_at_box(&boat_box).is_empty() {
            return;
        }

        // 创建并生成船
        let entity = Entity::new(world.clone(), hit_vec, entity_type);

        // 将偏航角设为玩家的偏航角
        let (player_yaw, _) = player.rotation();
        entity.set_rotation(player_yaw, 0.0);

        let boat_entity = Arc::new(BoatEntity::new(entity));
        world.spawn_entity(boat_entity);

        let mut main_hand = player.inventory.held_item();
        let consumed = if !main_hand.is_empty() && main_hand.item.id == item.id {
            main_hand.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_held_item(main_hand);
            true
        } else {
            false
        };

        if !consumed {
            let mut off_hand = player.inventory.off_hand_item();
            if !off_hand.is_empty() && off_hand.item.id == item.id {
                off_hand.decrement_unless_creative(player.gamemode.load(), 1);
                player
                    .inventory
                    .set_stack_in_hand(papokin_util::Hand::Left, off_hand);
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
