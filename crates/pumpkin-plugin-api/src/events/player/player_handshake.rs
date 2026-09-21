use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerHandshakeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a client sends a handshake packet. This event is
/// cancellable; cancelling disconnects the client immediately.
pub struct PlayerHandshakeEvent;
impl FromIntoEvent for PlayerHandshakeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerHandshakeEvent;
    type Data = PlayerHandshakeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerHandshakeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerHandshakeEvent(data)
    }
}
