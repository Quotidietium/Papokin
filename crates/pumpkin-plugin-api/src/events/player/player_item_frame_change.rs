use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerItemFrameChangeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player places, removes or rotates an item in
/// an item frame. This event is cancellable.
pub struct PlayerItemFrameChangeEvent;
impl FromIntoEvent for PlayerItemFrameChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemFrameChangeEvent;
    type Data = PlayerItemFrameChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemFrameChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemFrameChangeEvent(data)
    }
}
