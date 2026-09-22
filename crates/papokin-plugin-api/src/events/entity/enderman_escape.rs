use crate::wit::papokin::plugin::event::{EndermanEscapeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 末影人逃离战斗时触发的事件。
pub struct EndermanEscapeEvent;
impl FromIntoEvent for EndermanEscapeEvent {
    const EVENT_TYPE: EventType = EventType::EndermanEscapeEvent;
    type Data = EndermanEscapeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EndermanEscapeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EndermanEscapeEvent(data)
    }
}
