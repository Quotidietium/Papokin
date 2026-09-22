use crate::wit::papokin::plugin::event::{EntityTeleportEndGatewayEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体经末地折跃门传送时触发的事件。
pub struct EntityTeleportEndGatewayEvent;
impl FromIntoEvent for EntityTeleportEndGatewayEvent {
    const EVENT_TYPE: EventType = EventType::EntityTeleportEndGatewayEvent;
    type Data = EntityTeleportEndGatewayEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTeleportEndGatewayEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTeleportEndGatewayEvent(data)
    }
}
