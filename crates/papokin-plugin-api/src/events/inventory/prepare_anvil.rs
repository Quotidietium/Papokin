use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PrepareAnvilEventData};

/// 物品在铁砧上备料时触发的事件。
pub struct PrepareAnvilEvent;
impl FromIntoEvent for PrepareAnvilEvent {
    const EVENT_TYPE: EventType = EventType::PrepareAnvilEvent;
    type Data = PrepareAnvilEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrepareAnvilEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrepareAnvilEvent(data)
    }
}
