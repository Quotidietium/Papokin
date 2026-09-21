use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerInventorySlotChangeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when an item in a player's inventory slot changes.
pub struct PlayerInventorySlotChangeEvent;
impl FromIntoEvent for PlayerInventorySlotChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInventorySlotChangeEvent;
    type Data = PlayerInventorySlotChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInventorySlotChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInventorySlotChangeEvent(data)
    }
}
