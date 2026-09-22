use crate::wit::papokin::plugin::event::{Event, EventType, WorldDifficultyChangeEventData};

use super::super::FromIntoEvent;

/// 世界难度变化时触发的事件。
pub struct WorldDifficultyChangeEvent;
impl FromIntoEvent for WorldDifficultyChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldDifficultyChangeEvent;
    type Data = WorldDifficultyChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldDifficultyChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldDifficultyChangeEvent(data)
    }
}
