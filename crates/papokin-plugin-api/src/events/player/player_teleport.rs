use crate::wit::papokin::plugin::event::{Event, EventType, PlayerTeleportEventData};

use super::super::FromIntoEvent;

/// 玩家被传送时触发的事件。
///
/// 关联的 [`PlayerTeleportEventData`] 包含玩家、起始位置、
/// 以及目标位置。此事件可取消。
pub struct PlayerTeleportEvent;
impl FromIntoEvent for PlayerTeleportEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTeleportEvent;
    type Data = PlayerTeleportEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTeleportEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTeleportEvent(data)
    }
}
