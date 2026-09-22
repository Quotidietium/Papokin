use crate::wit::papokin::plugin::event::{BlockFailedDispenseEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 发射器发射物品失败时触发的事件。
pub struct BlockFailedDispenseEvent;
impl FromIntoEvent for BlockFailedDispenseEvent {
    const EVENT_TYPE: EventType = EventType::BlockFailedDispenseEvent;
    type Data = BlockFailedDispenseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockFailedDispenseEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockFailedDispenseEvent(data)
    }
}
