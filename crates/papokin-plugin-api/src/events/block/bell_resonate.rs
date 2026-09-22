use crate::wit::papokin::plugin::event::{BellResonateEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 钟共鸣时触发的事件。
pub struct BellResonateEvent;
impl FromIntoEvent for BellResonateEvent {
    const EVENT_TYPE: EventType = EventType::BellResonateEvent;
    type Data = BellResonateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BellResonateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BellResonateEvent(data)
    }
}
