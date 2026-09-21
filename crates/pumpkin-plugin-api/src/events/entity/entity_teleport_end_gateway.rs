use crate::wit::pumpkin::plugin::event::{EntityTeleportEndGatewayEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity teleports through an end gateway.
pub struct EntityTeleportEndGatewayEvent;
impl FromIntoEvent for EntityTeleportEndGatewayEvent {
    const EVENT_TYPE: EventType = EventType::EntityTeleportEndGatewayEvent;
    type Data = EntityTeleportEndGatewayEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTeleportEndGatewayEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTeleportEndGatewayEvent(data)
    }
}
