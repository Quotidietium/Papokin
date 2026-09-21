use crate::wit::pumpkin::plugin::event::{EntityEquipmentChangedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity's equipment in a slot changes.
pub struct EntityEquipmentChangedEvent;
impl FromIntoEvent for EntityEquipmentChangedEvent {
    const EVENT_TYPE: EventType = EventType::EntityEquipmentChangedEvent;
    type Data = EntityEquipmentChangedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityEquipmentChangedEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityEquipmentChangedEvent(data)
    }
}
