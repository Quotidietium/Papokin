use crate::wit::papokin::plugin::event::{EntityMountEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体骑乘另一实体时触发的事件。
pub struct EntityMountEvent;
impl FromIntoEvent for EntityMountEvent {
    const EVENT_TYPE: EventType = EventType::EntityMountEvent;
    type Data = EntityMountEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityMountEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityMountEvent(data)
    }
}
