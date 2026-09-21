use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerLecternPageChangeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player changes the page of a book on a
/// lectern. This event is cancellable; the new page may be modified.
pub struct PlayerLecternPageChangeEvent;
impl FromIntoEvent for PlayerLecternPageChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLecternPageChangeEvent;
    type Data = PlayerLecternPageChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLecternPageChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLecternPageChangeEvent(data)
    }
}
