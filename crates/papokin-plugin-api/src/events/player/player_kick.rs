use crate::wit::papokin::plugin::event::{Event, EventType, PlayerKickEventData};

use super::super::FromIntoEvent;

/// 玩家被踢出服务器时触发的事件。
pub struct PlayerKickEvent;
impl FromIntoEvent for PlayerKickEvent {
    const EVENT_TYPE: EventType = EventType::PlayerKickEvent;
    type Data = PlayerKickEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerKickEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerKickEvent(data)
    }
}
