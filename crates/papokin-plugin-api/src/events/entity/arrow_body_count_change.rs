use crate::wit::papokin::plugin::event::{ArrowBodyCountChangeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体身上插着的箭数量变化时触发的事件。
pub struct ArrowBodyCountChangeEvent;
impl FromIntoEvent for ArrowBodyCountChangeEvent {
    const EVENT_TYPE: EventType = EventType::ArrowBodyCountChangeEvent;
    type Data = ArrowBodyCountChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ArrowBodyCountChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ArrowBodyCountChangeEvent(data)
    }
}
