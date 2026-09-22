use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, TimeSkipEventData};

/// 世界时间被跳过时时触发的事件。
pub struct TimeSkipEvent;
impl FromIntoEvent for TimeSkipEvent {
    const EVENT_TYPE: EventType = EventType::TimeSkipEvent;
    type Data = TimeSkipEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::TimeSkipEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::TimeSkipEvent(data)
    }
}
