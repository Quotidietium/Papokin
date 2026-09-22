use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockDispenseEventData, Event, EventType};

/// 方块发射物品时触发的事件。
pub struct BlockDispenseEvent;
impl FromIntoEvent for BlockDispenseEvent {
    const EVENT_TYPE: EventType = EventType::BlockDispenseEvent;
    type Data = BlockDispenseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockDispenseEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockDispenseEvent(data)
    }
}
