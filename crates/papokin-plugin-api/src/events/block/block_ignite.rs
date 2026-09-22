use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockIgniteEventData, Event, EventType};

/// 方块被点燃时触发的事件。
pub struct BlockIgniteEvent;
impl FromIntoEvent for BlockIgniteEvent {
    const EVENT_TYPE: EventType = EventType::BlockIgniteEvent;
    type Data = BlockIgniteEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockIgniteEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockIgniteEvent(data)
    }
}
