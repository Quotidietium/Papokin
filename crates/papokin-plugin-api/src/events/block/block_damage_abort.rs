use crate::wit::papokin::plugin::event::{BlockDamageAbortEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家停止挖掘方块时触发的事件。
pub struct BlockDamageAbortEvent;
impl FromIntoEvent for BlockDamageAbortEvent {
    const EVENT_TYPE: EventType = EventType::BlockDamageAbortEvent;
    type Data = BlockDamageAbortEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockDamageAbortEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockDamageAbortEvent(data)
    }
}
