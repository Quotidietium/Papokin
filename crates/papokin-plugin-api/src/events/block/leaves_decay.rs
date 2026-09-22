use crate::wit::papokin::plugin::event::{Event, EventType, LeavesDecayEventData};

use super::super::FromIntoEvent;

/// 树叶自然枯萎时触发的事件。
pub struct LeavesDecayEvent;
impl FromIntoEvent for LeavesDecayEvent {
    const EVENT_TYPE: EventType = EventType::LeavesDecayEvent;
    type Data = LeavesDecayEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::LeavesDecayEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::LeavesDecayEvent(data)
    }
}
