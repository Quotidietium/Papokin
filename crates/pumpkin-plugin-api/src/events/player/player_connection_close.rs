use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerConnectionCloseEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player connection is closed.
pub struct PlayerConnectionCloseEvent;
impl FromIntoEvent for PlayerConnectionCloseEvent {
    const EVENT_TYPE: EventType = EventType::PlayerConnectionCloseEvent;
    type Data = PlayerConnectionCloseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerConnectionCloseEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerConnectionCloseEvent(data)
    }
}
