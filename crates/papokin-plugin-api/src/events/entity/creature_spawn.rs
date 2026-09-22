use crate::wit::papokin::plugin::event::{CreatureSpawnEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 生物生成时触发的事件。
pub struct CreatureSpawnEvent;
impl FromIntoEvent for CreatureSpawnEvent {
    const EVENT_TYPE: EventType = EventType::CreatureSpawnEvent;
    type Data = CreatureSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CreatureSpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CreatureSpawnEvent(data)
    }
}
