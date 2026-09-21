use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerItemCooldownEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a cooldown is applied to an item type for a
/// player. This event is cancellable.
pub struct PlayerItemCooldownEvent;
impl FromIntoEvent for PlayerItemCooldownEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemCooldownEvent;
    type Data = PlayerItemCooldownEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemCooldownEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemCooldownEvent(data)
    }
}
