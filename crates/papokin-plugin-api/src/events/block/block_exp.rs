use crate::wit::papokin::plugin::event::{BlockExpEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块给予经验时触发的事件。
pub struct BlockExpEvent;
impl FromIntoEvent for BlockExpEvent {
    const EVENT_TYPE: EventType = EventType::BlockExpEvent;
    type Data = BlockExpEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockExpEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockExpEvent(data)
    }
}
