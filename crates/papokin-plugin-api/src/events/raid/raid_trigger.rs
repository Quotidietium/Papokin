use crate::wit::papokin::plugin::event::{Event, EventType, RaidTriggerEventData};

use super::super::FromIntoEvent;

/// 袭击被触发时触发的事件。
pub struct RaidTriggerEvent;
impl FromIntoEvent for RaidTriggerEvent {
    const EVENT_TYPE: EventType = EventType::RaidTriggerEvent;
    type Data = RaidTriggerEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::RaidTriggerEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::RaidTriggerEvent(data)
    }
}
