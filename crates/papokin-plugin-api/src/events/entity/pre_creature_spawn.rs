use crate::wit::papokin::plugin::event::{Event, EventType, PreCreatureSpawnEventData};

use super::super::FromIntoEvent;

/// 生物生成前触发的事件，允许提前过滤。
pub struct PreCreatureSpawnEvent;
impl FromIntoEvent for PreCreatureSpawnEvent {
    const EVENT_TYPE: EventType = EventType::PreCreatureSpawnEvent;
    type Data = PreCreatureSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreCreatureSpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreCreatureSpawnEvent(data)
    }
}
