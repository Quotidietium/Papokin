use papokin_macros::Event;
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

use super::PlayerEvent;

/// 玩家重生时触发的事件。
///
/// 这是一个通知，在重生目的地（世界、位置等）确定后触发一次
/// 旋转）确定之后触发。不可取消。
///
/// 在跨维度重生时，可取消的
/// [`PlayerChangeWorldEvent`](super::player_change_world::PlayerChangeWorldEvent)
/// 先触发，发生在世界转移之前；此事件在转移之后触发
/// 并反映玩家最终实际所在的世界——包括
/// `PlayerChangeWorldEvent` 重定向与无操作回退（玩家留在
/// 之前的世界）（当该事件被取消时）。详见该事件的文档以了解
/// 修改与取消之间的语义；特别是，由
/// 已取消的 `PlayerChangeWorldEvent` 不会延续到此处。
#[derive(Event, Clone)]
pub struct PlayerRespawnEvent {
    /// 重生的玩家。
    pub player: Arc<Player>,

    /// 玩家重生前所在的世界（即死亡处）。
    pub previous_world: Arc<World>,

    /// 玩家重生后所在的世界。
    pub respawned_world: Arc<World>,

    /// 玩家重生所在的位置。
    pub position: Vector3<f64>,

    /// 玩家重生时的偏航角。
    pub yaw: f32,

    /// 玩家重生时的俯仰角。
    pub pitch: f32,

    /// 玩家是否保留了其数据（仍在……时重生则为 `true`
    /// 仍存活，例如离开末地时）。
    pub alive: bool,
}

impl PlayerRespawnEvent {
    /// 创建 `PlayerRespawnEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：发生重生的玩家。
    /// - `previous_world`：玩家重生前所在的世界。
    /// - `respawned_world`：玩家重生后所在的世界。
    /// - `position`：玩家重生时所在的位置。
    /// - `yaw`：玩家重生时的偏航角。
    /// - `pitch`：玩家重生时的俯仰角。
    /// - `alive`：玩家是否保留了其数据。
    ///
    /// # Returns
    /// 一个新的 `PlayerRespawnEvent` 实例。
    pub const fn new(
        player: Arc<Player>,
        previous_world: Arc<World>,
        respawned_world: Arc<World>,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        alive: bool,
    ) -> Self {
        Self {
            player,
            previous_world,
            respawned_world,
            position,
            yaw,
            pitch,
            alive,
        }
    }
}

impl PlayerEvent for PlayerRespawnEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
