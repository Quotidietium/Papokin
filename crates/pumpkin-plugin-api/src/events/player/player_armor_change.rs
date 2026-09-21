use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerArmorChangeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player's armor piece changes.
pub struct PlayerArmorChangeEvent;
impl FromIntoEvent for PlayerArmorChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerArmorChangeEvent;
    type Data = PlayerArmorChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerArmorChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerArmorChangeEvent(data)
    }
}
