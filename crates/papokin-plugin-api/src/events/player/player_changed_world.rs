use crate::wit::papokin::plugin::event::{Event, EventType, PlayerChangedWorldEventData};

use super::super::FromIntoEvent;

/// 玩家切换世界之后触发的事件。
pub struct PlayerChangedWorldEvent;
impl FromIntoEvent for PlayerChangedWorldEvent {
    const EVENT_TYPE: EventType = EventType::PlayerChangedWorldEvent;
    type Data = PlayerChangedWorldEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerChangedWorldEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerChangedWorldEvent(data)
    }
}
