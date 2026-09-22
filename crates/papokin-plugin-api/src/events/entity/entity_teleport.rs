use crate::wit::papokin::plugin::event::{EntityTeleportEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体传送时触发的事件。
pub struct EntityTeleportEvent;
impl FromIntoEvent for EntityTeleportEvent {
    const EVENT_TYPE: EventType = EventType::EntityTeleportEvent;
    type Data = EntityTeleportEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTeleportEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTeleportEvent(data)
    }
}
