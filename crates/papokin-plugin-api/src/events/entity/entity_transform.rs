use crate::wit::papokin::plugin::event::{EntityTransformEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体转变为另一实体时触发的事件。
pub struct EntityTransformEvent;
impl FromIntoEvent for EntityTransformEvent {
    const EVENT_TYPE: EventType = EventType::EntityTransformEvent;
    type Data = EntityTransformEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTransformEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTransformEvent(data)
    }
}
