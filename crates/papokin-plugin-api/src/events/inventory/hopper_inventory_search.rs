use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, HopperInventorySearchEventData};

/// 漏斗搜索容器时触发的事件。
pub struct HopperInventorySearchEvent;
impl FromIntoEvent for HopperInventorySearchEvent {
    const EVENT_TYPE: EventType = EventType::HopperInventorySearchEvent;
    type Data = HopperInventorySearchEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::HopperInventorySearchEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::HopperInventorySearchEvent(data)
    }
}
