use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, WorldSaveEventData};

/// 世界保存时触发的事件。
pub struct WorldSaveEvent;
impl FromIntoEvent for WorldSaveEvent {
    const EVENT_TYPE: EventType = EventType::WorldSaveEvent;
    type Data = WorldSaveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldSaveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldSaveEvent(data)
    }
}
