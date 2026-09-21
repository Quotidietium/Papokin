use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerTeleportEndGatewayEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player teleports through an end gateway. This
/// event is cancellable.
pub struct PlayerTeleportEndGatewayEvent;
impl FromIntoEvent for PlayerTeleportEndGatewayEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTeleportEndGatewayEvent;
    type Data = PlayerTeleportEndGatewayEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTeleportEndGatewayEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTeleportEndGatewayEvent(data)
    }
}
