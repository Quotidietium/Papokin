use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, SignChangeEventData};

/// 告示牌文本被修改时触发的事件。
pub struct SignChangeEvent;
impl FromIntoEvent for SignChangeEvent {
    const EVENT_TYPE: EventType = EventType::SignChangeEvent;
    type Data = SignChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SignChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SignChangeEvent(data)
    }
}
