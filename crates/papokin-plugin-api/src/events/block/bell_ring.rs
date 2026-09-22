use crate::wit::papokin::plugin::event::{BellRingEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 钟被敲响时触发的事件。
pub struct BellRingEvent;
impl FromIntoEvent for BellRingEvent {
    const EVENT_TYPE: EventType = EventType::BellRingEvent;
    type Data = BellRingEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BellRingEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BellRingEvent(data)
    }
}
