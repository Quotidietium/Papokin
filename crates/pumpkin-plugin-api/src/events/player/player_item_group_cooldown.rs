use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerItemGroupCooldownEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a cooldown is applied to an item cooldown group
/// for a player. This event is cancellable.
pub struct PlayerItemGroupCooldownEvent;
impl FromIntoEvent for PlayerItemGroupCooldownEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemGroupCooldownEvent;
    type Data = PlayerItemGroupCooldownEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemGroupCooldownEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemGroupCooldownEvent(data)
    }
}
