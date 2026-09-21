use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerFlowerPotManipulateEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player manipulates the content of a flower
/// pot. This event is cancellable.
pub struct PlayerFlowerPotManipulateEvent;
impl FromIntoEvent for PlayerFlowerPotManipulateEvent {
    const EVENT_TYPE: EventType = EventType::PlayerFlowerPotManipulateEvent;
    type Data = PlayerFlowerPotManipulateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerFlowerPotManipulateEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerFlowerPotManipulateEvent(data)
    }
}
