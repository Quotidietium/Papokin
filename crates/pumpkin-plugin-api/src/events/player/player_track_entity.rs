use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerTrackEntityEventData};

use super::super::FromIntoEvent;

/// An event that occurs when an entity starts being tracked (sent) to a
/// player.
pub struct PlayerTrackEntityEvent;
impl FromIntoEvent for PlayerTrackEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTrackEntityEvent;
    type Data = PlayerTrackEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTrackEntityEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTrackEntityEvent(data)
    }
}
