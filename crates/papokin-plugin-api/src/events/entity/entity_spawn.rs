use crate::wit::papokin::plugin::event::{EntitySpawnEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体在世界生成时触发的事件。
pub struct EntitySpawnEvent;
impl FromIntoEvent for EntitySpawnEvent {
    const EVENT_TYPE: EventType = EventType::EntitySpawnEvent;
    type Data = EntitySpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntitySpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntitySpawnEvent(data)
    }
}
