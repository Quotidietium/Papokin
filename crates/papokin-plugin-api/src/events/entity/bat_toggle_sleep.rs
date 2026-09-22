use crate::wit::papokin::plugin::event::{BatToggleSleepEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 蝙蝠切换睡眠状态时触发的事件。
pub struct BatToggleSleepEvent;
impl FromIntoEvent for BatToggleSleepEvent {
    const EVENT_TYPE: EventType = EventType::BatToggleSleepEvent;
    type Data = BatToggleSleepEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BatToggleSleepEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BatToggleSleepEvent(data)
    }
}
