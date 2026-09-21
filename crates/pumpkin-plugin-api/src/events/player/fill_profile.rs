use crate::wit::pumpkin::plugin::event::{Event, EventType, FillProfileEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player profile has been filled with its
/// properties.
pub struct FillProfileEvent;
impl FromIntoEvent for FillProfileEvent {
    const EVENT_TYPE: EventType = EventType::FillProfileEvent;
    type Data = FillProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FillProfileEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FillProfileEvent(data)
    }
}
