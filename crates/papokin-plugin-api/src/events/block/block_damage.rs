use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockDamageEventData, Event, EventType};

/// 方块被玩家挖掘时触发的事件。
pub struct BlockDamageEvent;
impl FromIntoEvent for BlockDamageEvent {
    const EVENT_TYPE: EventType = EventType::BlockDamageEvent;
    type Data = BlockDamageEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockDamageEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockDamageEvent(data)
    }
}
