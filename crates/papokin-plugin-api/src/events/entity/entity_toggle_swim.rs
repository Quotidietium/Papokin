use crate::wit::papokin::plugin::event::{EntityToggleSwimEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体开始或停止游泳时触发的事件。
pub struct EntityToggleSwimEvent;
impl FromIntoEvent for EntityToggleSwimEvent {
    const EVENT_TYPE: EventType = EventType::EntityToggleSwimEvent;
    type Data = EntityToggleSwimEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityToggleSwimEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityToggleSwimEvent(data)
    }
}
