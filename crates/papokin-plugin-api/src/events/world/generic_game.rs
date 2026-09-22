use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, GenericGameEventData};

/// 通用游戏事件。
pub struct GenericGameEvent;
impl FromIntoEvent for GenericGameEvent {
    const EVENT_TYPE: EventType = EventType::GenericGameEvent;
    type Data = GenericGameEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::GenericGameEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::GenericGameEvent(data)
    }
}
