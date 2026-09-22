use crate::wit::papokin::plugin::event::{Event, EventType, WardenAngerChangeEventData};

use super::super::FromIntoEvent;

/// 监守者对实体的愤怒值变化时触发的事件。
pub struct WardenAngerChangeEvent;
impl FromIntoEvent for WardenAngerChangeEvent {
    const EVENT_TYPE: EventType = EventType::WardenAngerChangeEvent;
    type Data = WardenAngerChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WardenAngerChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WardenAngerChangeEvent(data)
    }
}
