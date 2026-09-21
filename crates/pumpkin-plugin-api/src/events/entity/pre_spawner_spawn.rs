use crate::wit::pumpkin::plugin::event::{Event, EventType, PreSpawnerSpawnEventData};

use super::super::FromIntoEvent;

/// Event triggered before a spawner spawns an entity.
pub struct PreSpawnerSpawnEvent;
impl FromIntoEvent for PreSpawnerSpawnEvent {
    const EVENT_TYPE: EventType = EventType::PreSpawnerSpawnEvent;
    type Data = PreSpawnerSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreSpawnerSpawnEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreSpawnerSpawnEvent(data)
    }
}
