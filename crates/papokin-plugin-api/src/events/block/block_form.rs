use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockFormEventData, Event, EventType};

/// 方块形成时触发的事件。
pub struct BlockFormEvent;
impl FromIntoEvent for BlockFormEvent {
    const EVENT_TYPE: EventType = EventType::BlockFormEvent;
    type Data = BlockFormEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockFormEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockFormEvent(data)
    }
}
