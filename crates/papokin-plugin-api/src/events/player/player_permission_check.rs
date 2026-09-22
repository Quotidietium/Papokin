use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPermissionCheckEventData};

use super::super::FromIntoEvent;

/// 对玩家执行权限检查时触发的事件。
///
/// 关联的 [`PlayerPermissionCheckEventData`] 包含玩家、权限
/// 节点，以及可被覆盖的当前结果。
pub struct PlayerPermissionCheckEvent;
impl FromIntoEvent for PlayerPermissionCheckEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPermissionCheckEvent;
    type Data = PlayerPermissionCheckEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPermissionCheckEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPermissionCheckEvent(data)
    }
}
