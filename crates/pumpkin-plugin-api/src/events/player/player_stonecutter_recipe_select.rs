use crate::wit::pumpkin::plugin::event::{
    Event, EventType, PlayerStonecutterRecipeSelectEventData,
};

use super::super::FromIntoEvent;

/// An event that occurs when a player selects a recipe in a stonecutter. This
/// event is cancellable.
pub struct PlayerStonecutterRecipeSelectEvent;
impl FromIntoEvent for PlayerStonecutterRecipeSelectEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStonecutterRecipeSelectEvent;
    type Data = PlayerStonecutterRecipeSelectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStonecutterRecipeSelectEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStonecutterRecipeSelectEvent(data)
    }
}
