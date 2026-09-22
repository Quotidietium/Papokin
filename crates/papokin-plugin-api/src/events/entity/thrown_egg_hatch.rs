use crate::wit::papokin::plugin::event::{Event, EventType, ThrownEggHatchEventData};

use super::super::FromIntoEvent;

/// 掷出的鸡蛋决定是否孵化时触发的事件。
pub struct ThrownEggHatchEvent;
impl FromIntoEvent for ThrownEggHatchEvent {
    const EVENT_TYPE: EventType = EventType::ThrownEggHatchEvent;
    type Data = ThrownEggHatchEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ThrownEggHatchEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ThrownEggHatchEvent(data)
    }
}
