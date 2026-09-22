use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{
    Event, EventType, WorldLoadEventData, WorldUnloadEventData,
};

/// 世界加载时触发的事件。
pub struct WorldLoadEvent;
impl FromIntoEvent for WorldLoadEvent {
    const EVENT_TYPE: EventType = EventType::WorldLoadEvent;
    type Data = WorldLoadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldLoadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldLoadEvent(data)
    }
}

/// 世界卸载时触发的事件。
pub struct WorldUnloadEvent;
impl FromIntoEvent for WorldUnloadEvent {
    const EVENT_TYPE: EventType = EventType::WorldUnloadEvent;
    type Data = WorldUnloadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldUnloadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldUnloadEvent(data)
    }
}
