use crate::wit::pumpkin::plugin::event::{EntityToggleSitEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity toggles its sitting state.
pub struct EntityToggleSitEvent;
impl FromIntoEvent for EntityToggleSitEvent {
    const EVENT_TYPE: EventType = EventType::EntityToggleSitEvent;
    type Data = EntityToggleSitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityToggleSitEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityToggleSitEvent(data)
    }
}
