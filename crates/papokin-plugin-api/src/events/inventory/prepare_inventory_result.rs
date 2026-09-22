use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PrepareInventoryResultEventData};

/// 物品栏结果槽备料时触发的通用事件。
pub struct PrepareInventoryResultEvent;
impl FromIntoEvent for PrepareInventoryResultEvent {
    const EVENT_TYPE: EventType = EventType::PrepareInventoryResultEvent;
    type Data = PrepareInventoryResultEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrepareInventoryResultEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrepareInventoryResultEvent(data)
    }
}
