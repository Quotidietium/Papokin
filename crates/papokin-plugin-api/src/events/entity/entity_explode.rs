use crate::wit::papokin::plugin::event::{EntityExplodeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体爆炸时触发的事件。
pub struct EntityExplodeEvent;
impl FromIntoEvent for EntityExplodeEvent {
    const EVENT_TYPE: EventType = EventType::EntityExplodeEvent;
    type Data = EntityExplodeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityExplodeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityExplodeEvent(data)
    }
}
