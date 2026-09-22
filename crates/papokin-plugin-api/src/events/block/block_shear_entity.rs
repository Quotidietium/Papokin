use crate::wit::papokin::plugin::event::{BlockShearEntityEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 发射器剪实体时触发的事件。
pub struct BlockShearEntityEvent;
impl FromIntoEvent for BlockShearEntityEvent {
    const EVENT_TYPE: EventType = EventType::BlockShearEntityEvent;
    type Data = BlockShearEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockShearEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockShearEntityEvent(data)
    }
}
