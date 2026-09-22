use crate::wit::papokin::plugin::event::{EntityPortalExitEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体离开传送门时触发的事件。
pub struct EntityPortalExitEvent;
impl FromIntoEvent for EntityPortalExitEvent {
    const EVENT_TYPE: EventType = EventType::EntityPortalExitEvent;
    type Data = EntityPortalExitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPortalExitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPortalExitEvent(data)
    }
}
