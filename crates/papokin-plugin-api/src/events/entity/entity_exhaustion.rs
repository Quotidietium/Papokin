use crate::wit::papokin::plugin::event::{EntityExhaustionEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体产生饥饿消耗时触发的事件。
pub struct EntityExhaustionEvent;
impl FromIntoEvent for EntityExhaustionEvent {
    const EVENT_TYPE: EventType = EventType::EntityExhaustionEvent;
    type Data = EntityExhaustionEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityExhaustionEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityExhaustionEvent(data)
    }
}
