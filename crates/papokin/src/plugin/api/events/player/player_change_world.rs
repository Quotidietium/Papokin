use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

use super::PlayerEvent;

/// 玩家被传送到另一个世界时发生的事件。
///
/// `new_world` 与位置/偏航/俯仰字段是可变的；当未
/// 取消，修改后的值会按原样应用于传送，并且
/// 随后会触发 [`PlayerRespawnEvent`](super::player_respawn::PlayerRespawnEvent)。
/// 取消将丢弃整个事件：世界保持不变，任何编辑
/// 都会被丢弃，重生将使用事件触发前解析的值
/// 触发。若要让玩家留在当前世界的选定位置，请不要
/// 取消——将 `new_world` 设为 `previous_world`（UUID 匹配）并覆盖
/// 位置/偏航角/俯仰角字段；传送会被跳过，但这些值
/// 生效。重定向到另一个世界时，应设置位置/偏航角/俯仰角
/// 对其有效。
///
/// 在跨维度重生时，此事件先于不可取消的
/// [`PlayerRespawnEvent`](super::player_respawn::PlayerRespawnEvent)，该事件
/// 然后观察已解析的世界。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerChangeWorldEvent {
    /// 正在传送到另一个世界的玩家。
    pub player: Arc<Player>,

    /// 玩家之前所在的世界。
    pub previous_world: Arc<World>,

    /// 玩家所在的新世界。
    pub new_world: Arc<World>,

    /// 玩家被传送到的位置。
    pub position: Vector3<f64>,

    /// 传送后玩家的偏航角。
    pub yaw: f32,

    /// 传送后玩家的俯仰角。
    pub pitch: f32,
}

impl PlayerChangeWorldEvent {
    /// 创建 `PlayerChangeWorldEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在切换世界的玩家的引用。
    /// - `previous_world`：玩家先前所在的世界。
    /// - `new_world`：玩家所在的新世界。
    /// - `position`：玩家被传送到的位置。
    /// - `yaw`：传送后玩家的偏航角。
    /// - `pitch`：传送后玩家的俯仰角。
    ///
    /// # Returns
    /// 一个新的 `PlayerChangeWorldEvent` 实例。
    pub const fn new(
        player: Arc<Player>,
        previous_world: Arc<World>,
        new_world: Arc<World>,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
    ) -> Self {
        Self {
            player,
            previous_world,
            new_world,
            position,
            yaw,
            pitch,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerChangeWorldEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
