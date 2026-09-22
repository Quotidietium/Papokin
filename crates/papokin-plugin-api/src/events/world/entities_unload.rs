use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{EntitiesUnloadEventData, Event, EventType};

/// 实体随区块卸载时触发的事件。
pub struct EntitiesUnloadEvent;
impl FromIntoEvent for EntitiesUnloadEvent {
    const EVENT_TYPE: EventType = EventType::EntitiesUnloadEvent;
    type Data = EntitiesUnloadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntitiesUnloadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntitiesUnloadEvent(data)
    }
}
