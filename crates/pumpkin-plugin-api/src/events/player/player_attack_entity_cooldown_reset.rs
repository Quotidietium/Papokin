use crate::wit::pumpkin::plugin::event::{
    Event, EventType, PlayerAttackEntityCooldownResetEventData,
};

use super::super::FromIntoEvent;

/// An event that occurs when a player's attack cooldown is reset by attacking
/// an entity. This event is cancellable.
pub struct PlayerAttackEntityCooldownResetEvent;
impl FromIntoEvent for PlayerAttackEntityCooldownResetEvent {
    const EVENT_TYPE: EventType = EventType::PlayerAttackEntityCooldownResetEvent;
    type Data = PlayerAttackEntityCooldownResetEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerAttackEntityCooldownResetEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerAttackEntityCooldownResetEvent(data)
    }
}
