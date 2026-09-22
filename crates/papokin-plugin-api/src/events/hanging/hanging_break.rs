use crate::wit::papokin::plugin::event::{Event, EventType, HangingBreakEventData};

use super::super::FromIntoEvent;

/// 悬挂实体被破坏时触发的事件。
pub struct HangingBreakEvent;
impl FromIntoEvent for HangingBreakEvent {
    const EVENT_TYPE: EventType = EventType::HangingBreakEvent;
    type Data = HangingBreakEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::HangingBreakEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::HangingBreakEvent(data)
    }
}
