use crate::wit::papokin::plugin::event::{Event, EventType, RaidFinishEventData};

use super::super::FromIntoEvent;

/// 袭击结束时触发的事件。
pub struct RaidFinishEvent;
impl FromIntoEvent for RaidFinishEvent {
    const EVENT_TYPE: EventType = EventType::RaidFinishEvent;
    type Data = RaidFinishEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::RaidFinishEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::RaidFinishEvent(data)
    }
}
