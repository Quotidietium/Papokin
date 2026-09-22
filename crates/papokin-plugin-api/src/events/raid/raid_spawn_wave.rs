use crate::wit::papokin::plugin::event::{Event, EventType, RaidSpawnWaveEventData};

use super::super::FromIntoEvent;

/// 一波袭击生成时触发的事件。
pub struct RaidSpawnWaveEvent;
impl FromIntoEvent for RaidSpawnWaveEvent {
    const EVENT_TYPE: EventType = EventType::RaidSpawnWaveEvent;
    type Data = RaidSpawnWaveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::RaidSpawnWaveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::RaidSpawnWaveEvent(data)
    }
}
