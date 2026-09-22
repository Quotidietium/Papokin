use crate::wit::papokin::plugin::event::{BlockBreakProgressUpdateEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 为玩家更新方块破坏进度时触发的事件。
pub struct BlockBreakProgressUpdateEvent;
impl FromIntoEvent for BlockBreakProgressUpdateEvent {
    const EVENT_TYPE: EventType = EventType::BlockBreakProgressUpdateEvent;
    type Data = BlockBreakProgressUpdateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockBreakProgressUpdateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockBreakProgressUpdateEvent(data)
    }
}
