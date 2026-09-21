use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerSignCommandPreprocessEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a command on a sign is preprocessed before
/// execution. This event is cancellable; the command may be modified.
pub struct PlayerSignCommandPreprocessEvent;
impl FromIntoEvent for PlayerSignCommandPreprocessEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSignCommandPreprocessEvent;
    type Data = PlayerSignCommandPreprocessEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSignCommandPreprocessEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSignCommandPreprocessEvent(data)
    }
}
