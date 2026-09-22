use crate::wit::papokin::plugin::event::{EntityPortalEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体进入传送门时触发的事件。
pub struct EntityPortalEvent;
impl FromIntoEvent for EntityPortalEvent {
    const EVENT_TYPE: EventType = EventType::EntityPortalEvent;
    type Data = EntityPortalEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPortalEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPortalEvent(data)
    }
}
