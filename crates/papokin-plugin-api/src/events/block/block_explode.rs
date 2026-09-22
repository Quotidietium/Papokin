use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockExplodeEventData, Event, EventType};

/// 方块爆炸时触发的事件。
pub struct BlockExplodeEvent;
impl FromIntoEvent for BlockExplodeEvent {
    const EVENT_TYPE: EventType = EventType::BlockExplodeEvent;
    type Data = BlockExplodeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockExplodeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockExplodeEvent(data)
    }
}
