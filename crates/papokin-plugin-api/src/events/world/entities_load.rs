use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{EntitiesLoadEventData, Event, EventType};

/// 实体随区块加载时触发的事件。
pub struct EntitiesLoadEvent;
impl FromIntoEvent for EntitiesLoadEvent {
    const EVENT_TYPE: EventType = EventType::EntitiesLoadEvent;
    type Data = EntitiesLoadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntitiesLoadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntitiesLoadEvent(data)
    }
}
