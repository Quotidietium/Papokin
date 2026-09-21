use crate::wit::pumpkin::plugin::event::{Event, EventType, ThrownEggHatchEventData};

use super::super::FromIntoEvent;

/// Event triggered when a thrown egg decides whether to hatch.
pub struct ThrownEggHatchEvent;
impl FromIntoEvent for ThrownEggHatchEvent {
    const EVENT_TYPE: EventType = EventType::ThrownEggHatchEvent;
    type Data = ThrownEggHatchEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ThrownEggHatchEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ThrownEggHatchEvent(data)
    }
}
