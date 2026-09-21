use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerLoomPatternSelectEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player selects a pattern in a loom. This event
/// is cancellable.
pub struct PlayerLoomPatternSelectEvent;
impl FromIntoEvent for PlayerLoomPatternSelectEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLoomPatternSelectEvent;
    type Data = PlayerLoomPatternSelectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLoomPatternSelectEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLoomPatternSelectEvent(data)
    }
}
