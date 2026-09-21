use crate::wit::pumpkin::plugin::event::{Event, EventType, StructuresLocateEventData};

use super::super::FromIntoEvent;

/// Event triggered when the server locates structures, e.g. for the locate command.
pub struct StructuresLocateEvent;
impl FromIntoEvent for StructuresLocateEvent {
    const EVENT_TYPE: EventType = EventType::StructuresLocateEvent;
    type Data = StructuresLocateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::StructuresLocateEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::StructuresLocateEvent(data)
    }
}
