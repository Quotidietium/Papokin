use crate::wit::papokin::plugin::event::{Event, EventType, MapInitializeEventData};

use super::super::FromIntoEvent;

/// 地图初始化时触发的事件。
pub struct MapInitializeEvent;
impl FromIntoEvent for MapInitializeEvent {
    const EVENT_TYPE: EventType = EventType::MapInitializeEvent;
    type Data = MapInitializeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::MapInitializeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::MapInitializeEvent(data)
    }
}
