use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PrepareGrindstoneEventData};

/// 物品在砂轮上备料时触发的事件。
pub struct PrepareGrindstoneEvent;
impl FromIntoEvent for PrepareGrindstoneEvent {
    const EVENT_TYPE: EventType = EventType::PrepareGrindstoneEvent;
    type Data = PrepareGrindstoneEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrepareGrindstoneEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrepareGrindstoneEvent(data)
    }
}
