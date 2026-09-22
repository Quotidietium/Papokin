use crate::wit::papokin::plugin::event::{Event, EventType, PlayerMoveEventData};

use super::super::FromIntoEvent;

/// 玩家移动时触发的事件。
///
/// 关联的 [`PlayerMoveEventData`] 包含玩家、移动起始位置、
/// 以及移动到的位置。此事件可取消。
pub struct PlayerMoveEvent;
impl FromIntoEvent for PlayerMoveEvent {
    const EVENT_TYPE: EventType = EventType::PlayerMoveEvent;
    type Data = PlayerMoveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerMoveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerMoveEvent(data)
    }
}
