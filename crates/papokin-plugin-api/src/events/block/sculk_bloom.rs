use crate::wit::papokin::plugin::event::{Event, EventType, SculkBloomEventData};

use super::super::FromIntoEvent;

/// 幽匿催发体催发时触发的事件。
pub struct SculkBloomEvent;
impl FromIntoEvent for SculkBloomEvent {
    const EVENT_TYPE: EventType = EventType::SculkBloomEvent;
    type Data = SculkBloomEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SculkBloomEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SculkBloomEvent(data)
    }
}
