use crate::wit::papokin::plugin::event::{Event, EventType, SpawnerSpawnEventData};

use super::super::FromIntoEvent;

/// 刷怪笼生成实体时触发的事件。
pub struct SpawnerSpawnEvent;
impl FromIntoEvent for SpawnerSpawnEvent {
    const EVENT_TYPE: EventType = EventType::SpawnerSpawnEvent;
    type Data = SpawnerSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SpawnerSpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SpawnerSpawnEvent(data)
    }
}
