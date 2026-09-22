use crate::wit::papokin::plugin::event::{Event, EventType, PreSpawnerSpawnEventData};

use super::super::FromIntoEvent;

/// 刷怪笼生成实体前触发的事件。
pub struct PreSpawnerSpawnEvent;
impl FromIntoEvent for PreSpawnerSpawnEvent {
    const EVENT_TYPE: EventType = EventType::PreSpawnerSpawnEvent;
    type Data = PreSpawnerSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreSpawnerSpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreSpawnerSpawnEvent(data)
    }
}
