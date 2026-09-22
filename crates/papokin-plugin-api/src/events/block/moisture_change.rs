use crate::wit::papokin::plugin::event::{Event, EventType, MoistureChangeEventData};

use super::super::FromIntoEvent;

/// 耕地湿度变化时触发的事件。
pub struct MoistureChangeEvent;
impl FromIntoEvent for MoistureChangeEvent {
    const EVENT_TYPE: EventType = EventType::MoistureChangeEvent;
    type Data = MoistureChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::MoistureChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::MoistureChangeEvent(data)
    }
}
