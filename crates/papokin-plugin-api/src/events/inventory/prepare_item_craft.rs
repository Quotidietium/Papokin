use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PrepareItemCraftEventData};

/// 配方在合成矩阵中备料时触发的事件。
pub struct PrepareItemCraftEvent;
impl FromIntoEvent for PrepareItemCraftEvent {
    const EVENT_TYPE: EventType = EventType::PrepareItemCraftEvent;
    type Data = PrepareItemCraftEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrepareItemCraftEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrepareItemCraftEvent(data)
    }
}
