use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, TntPrimeEventData};

/// TNT 被点燃时触发的事件。
pub struct TNTPrimeEvent;
impl FromIntoEvent for TNTPrimeEvent {
    const EVENT_TYPE: EventType = EventType::TntPrimeEvent;
    type Data = TntPrimeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::TntPrimeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::TntPrimeEvent(data)
    }
}
