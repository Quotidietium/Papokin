use crate::wit::pumpkin::plugin::event::{EntityZapEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity is zapped by lightning.
pub struct EntityZapEvent;
impl FromIntoEvent for EntityZapEvent {
    const EVENT_TYPE: EventType = EventType::EntityZapEvent;
    type Data = EntityZapEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityZapEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityZapEvent(data)
    }
}
