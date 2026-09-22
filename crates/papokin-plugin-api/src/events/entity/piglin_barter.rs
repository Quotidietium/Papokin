use crate::wit::papokin::plugin::event::{Event, EventType, PiglinBarterEventData};

use super::super::FromIntoEvent;

/// 猪灵以物易物时触发的事件。
pub struct PiglinBarterEvent;
impl FromIntoEvent for PiglinBarterEvent {
    const EVENT_TYPE: EventType = EventType::PiglinBarterEvent;
    type Data = PiglinBarterEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PiglinBarterEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PiglinBarterEvent(data)
    }
}
