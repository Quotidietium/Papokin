use crate::wit::papokin::plugin::event::{Event, EventType, StriderTemperatureChangeEventData};

use super::super::FromIntoEvent;

/// 炽足兽发抖状态变化时触发的事件。
pub struct StriderTemperatureChangeEvent;
impl FromIntoEvent for StriderTemperatureChangeEvent {
    const EVENT_TYPE: EventType = EventType::StriderTemperatureChangeEvent;
    type Data = StriderTemperatureChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::StriderTemperatureChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::StriderTemperatureChangeEvent(data)
    }
}
