use crate::wit::papokin::plugin::event::{EntityBreakDoorEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体破坏门时触发的事件。
pub struct EntityBreakDoorEvent;
impl FromIntoEvent for EntityBreakDoorEvent {
    const EVENT_TYPE: EventType = EventType::EntityBreakDoorEvent;
    type Data = EntityBreakDoorEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityBreakDoorEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityBreakDoorEvent(data)
    }
}
