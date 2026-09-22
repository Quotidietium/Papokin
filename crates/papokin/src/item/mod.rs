pub mod items;
pub mod potion;
pub mod registry;

use std::any::Any;
use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::server::Server;
use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub trait ItemMetadata {
    fn ids() -> Box<[u16]>;
}

pub trait ItemBehaviour: Send + Sync {
    fn normal_use(&self, _item: &Item, _player: &Player) {}

    /// 处理物品使用，并采用该操作上报的旋转角度。
    ///
    /// Java 客户端会在使用物品数据包中包含此旋转。物品行为
    /// 会执行射线检测的实体应重写此方法，而不是依赖
    /// 玩家可能过时的实体旋转。
    fn normal_use_with_rotation(&self, item: &Item, player: &Player, _yaw: f32, _pitch: f32) {
        self.normal_use(item, player);
    }

    #[expect(clippy::too_many_arguments)]
    fn use_on_block(
        &self,
        _item: &mut ItemStack,
        _player: &Player,
        _location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        _block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        BlockActionResult::Pass
    }

    fn use_on_entity(&self, _item: &mut ItemStack, _player: &Player, _entity: Arc<dyn EntityBase>) {
    }

    fn on_stopped_using(&self, _stack: &ItemStack, _player: &Player) {}

    fn on_spear_jab(&self, _stack: &ItemStack, _player: &Player) {}

    fn on_use_tick(&self, _stack: &ItemStack, _player: &Player, _remaining_use_ticks: i32) {}

    /// 返回此物品可使用的最大刻数。
    /// 若物品没有由行为决定的使用时长，则返回 0。
    fn get_use_duration(&self) -> i32 {
        0
    }

    fn can_mine(&self, _player: &Player) -> bool {
        true
    }

    fn get_start_and_end_pos(&self, player: &Player) -> (Vector3<f64>, Vector3<f64>) {
        let start_pos = player.eye_position();
        let (yaw, pitch) = player.rotation();
        let (yaw_rad, pitch_rad) = (f64::from(yaw.to_radians()), f64::from(pitch.to_radians()));
        let block_interaction_range = 4.5; // 这并不等同于下文中的 block_interaction_range，
        // 玩家实体。
        let direction = Vector3::new(
            -yaw_rad.sin() * pitch_rad.cos() * block_interaction_range,
            -pitch_rad.sin() * block_interaction_range,
            pitch_rad.cos() * yaw_rad.cos() * block_interaction_range,
        );

        let end_pos = start_pos.add(&direction);
        (start_pos, end_pos)
    }

    fn as_any(&self) -> &dyn Any;
}
