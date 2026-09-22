use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, WorldInitEventData};

/// 世界初始化时触发的事件。
pub struct WorldInitEvent;
impl FromIntoEvent for WorldInitEvent {
    const EVENT_TYPE: EventType = EventType::WorldInitEvent;
    type Data = WorldInitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldInitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldInitEvent(data)
    }
}
