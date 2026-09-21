use crate::wit::pumpkin::plugin::event::{EntityDamageItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an item held or worn by an entity takes durability damage.
pub struct EntityDamageItemEvent;
impl FromIntoEvent for EntityDamageItemEvent {
    const EVENT_TYPE: EventType = EventType::EntityDamageItemEvent;
    type Data = EntityDamageItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDamageItemEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDamageItemEvent(data)
    }
}
