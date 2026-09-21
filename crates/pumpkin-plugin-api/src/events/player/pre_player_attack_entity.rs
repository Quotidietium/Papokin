use crate::wit::pumpkin::plugin::event::{Event, EventType, PrePlayerAttackEntityEventData};

use super::super::FromIntoEvent;

/// An event that occurs before a player attacks an entity, allowing the
/// attack to be redirected or cancelled. This event is cancellable.
pub struct PrePlayerAttackEntityEvent;
impl FromIntoEvent for PrePlayerAttackEntityEvent {
    const EVENT_TYPE: EventType = EventType::PrePlayerAttackEntityEvent;
    type Data = PrePlayerAttackEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrePlayerAttackEntityEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrePlayerAttackEntityEvent(data)
    }
}
