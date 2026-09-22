use crate::wit::papokin::plugin::event::{Event, EventType, RaidStopEventData};

use super::super::FromIntoEvent;

/// 袭击停止时触发的事件。
pub struct RaidStopEvent;
impl FromIntoEvent for RaidStopEvent {
    const EVENT_TYPE: EventType = EventType::RaidStopEvent;
    type Data = RaidStopEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::RaidStopEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::RaidStopEvent(data)
    }
}
