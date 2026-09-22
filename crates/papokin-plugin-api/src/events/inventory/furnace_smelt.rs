use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, FurnaceSmeltEventData};

/// 物品在熔炉中熔炼时触发的事件。
pub struct FurnaceSmeltEvent;
impl FromIntoEvent for FurnaceSmeltEvent {
    const EVENT_TYPE: EventType = EventType::FurnaceSmeltEvent;
    type Data = FurnaceSmeltEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FurnaceSmeltEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FurnaceSmeltEvent(data)
    }
}
