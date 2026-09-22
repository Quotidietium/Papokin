use crate::wit::papokin::plugin::event::{BlockSpreadEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块基于世界条件蔓延时触发的事件。
pub struct BlockSpreadEvent;
impl FromIntoEvent for BlockSpreadEvent {
    const EVENT_TYPE: EventType = EventType::BlockSpreadEvent;
    type Data = BlockSpreadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockSpreadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockSpreadEvent(data)
    }
}
