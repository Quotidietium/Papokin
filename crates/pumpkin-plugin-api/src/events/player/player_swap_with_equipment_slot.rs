use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerSwapWithEquipmentSlotEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player swaps an item with an equipment slot.
/// This event is cancellable.
pub struct PlayerSwapWithEquipmentSlotEvent;
impl FromIntoEvent for PlayerSwapWithEquipmentSlotEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSwapWithEquipmentSlotEvent;
    type Data = PlayerSwapWithEquipmentSlotEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSwapWithEquipmentSlotEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSwapWithEquipmentSlotEvent(data)
    }
}
