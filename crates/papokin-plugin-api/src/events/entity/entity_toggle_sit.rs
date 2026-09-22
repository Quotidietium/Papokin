use crate::wit::papokin::plugin::event::{EntityToggleSitEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体切换坐下状态时触发的事件。
pub struct EntityToggleSitEvent;
impl FromIntoEvent for EntityToggleSitEvent {
    const EVENT_TYPE: EventType = EventType::EntityToggleSitEvent;
    type Data = EntityToggleSitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityToggleSitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityToggleSitEvent(data)
    }
}
