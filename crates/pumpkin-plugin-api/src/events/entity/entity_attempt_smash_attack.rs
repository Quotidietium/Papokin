use crate::wit::pumpkin::plugin::event::{EntityAttemptSmashAttackEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity attempts a smash attack.
pub struct EntityAttemptSmashAttackEvent;
impl FromIntoEvent for EntityAttemptSmashAttackEvent {
    const EVENT_TYPE: EventType = EventType::EntityAttemptSmashAttackEvent;
    type Data = EntityAttemptSmashAttackEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityAttemptSmashAttackEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityAttemptSmashAttackEvent(data)
    }
}
