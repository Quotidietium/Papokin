use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, FurnaceStartSmeltEventData};

/// 熔炉开始熔炼物品时触发的事件。
pub struct FurnaceStartSmeltEvent;
impl FromIntoEvent for FurnaceStartSmeltEvent {
    const EVENT_TYPE: EventType = EventType::FurnaceStartSmeltEvent;
    type Data = FurnaceStartSmeltEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FurnaceStartSmeltEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FurnaceStartSmeltEvent(data)
    }
}
