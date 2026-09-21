use crate::wit::pumpkin::plugin::event::{Event, EventType, ServerResourcesReloadedEventData};

use super::super::FromIntoEvent;

/// Event triggered when the server resources (data packs) are reloaded.
pub struct ServerResourcesReloadedEvent;
impl FromIntoEvent for ServerResourcesReloadedEvent {
    const EVENT_TYPE: EventType = EventType::ServerResourcesReloadedEvent;
    type Data = ServerResourcesReloadedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerResourcesReloadedEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerResourcesReloadedEvent(data)
    }
}
