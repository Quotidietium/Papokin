use crate::wit::papokin::plugin::event::{EntityPortalEnterEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体进入传送门时触发的事件。
pub struct EntityPortalEnterEvent;
impl FromIntoEvent for EntityPortalEnterEvent {
    const EVENT_TYPE: EventType = EventType::EntityPortalEnterEvent;
    type Data = EntityPortalEnterEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPortalEnterEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPortalEnterEvent(data)
    }
}
