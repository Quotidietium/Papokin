use crate::wit::papokin::plugin::event::{EntityUnleashEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被解开拴绳时触发的事件。
pub struct EntityUnleashEvent;
impl FromIntoEvent for EntityUnleashEvent {
    const EVENT_TYPE: EventType = EventType::EntityUnleashEvent;
    type Data = EntityUnleashEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityUnleashEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityUnleashEvent(data)
    }
}
