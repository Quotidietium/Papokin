use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, FurnaceBurnEventData};

/// 熔炉燃料燃烧时触发的事件。
pub struct FurnaceBurnEvent;
impl FromIntoEvent for FurnaceBurnEvent {
    const EVENT_TYPE: EventType = EventType::FurnaceBurnEvent;
    type Data = FurnaceBurnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FurnaceBurnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FurnaceBurnEvent(data)
    }
}
