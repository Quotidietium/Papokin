use crate::wit::papokin::plugin::event::{EntityToggleGlideEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体切换滑翔时触发的事件。
pub struct EntityToggleGlideEvent;
impl FromIntoEvent for EntityToggleGlideEvent {
    const EVENT_TYPE: EventType = EventType::EntityToggleGlideEvent;
    type Data = EntityToggleGlideEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityToggleGlideEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityToggleGlideEvent(data)
    }
}
