use crate::wit::papokin::plugin::event::{Event, EventType, SlimeSplitEventData};

use super::super::FromIntoEvent;

/// 史莱姆分裂为更小史莱姆时触发的事件。
pub struct SlimeSplitEvent;
impl FromIntoEvent for SlimeSplitEvent {
    const EVENT_TYPE: EventType = EventType::SlimeSplitEvent;
    type Data = SlimeSplitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SlimeSplitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SlimeSplitEvent(data)
    }
}
