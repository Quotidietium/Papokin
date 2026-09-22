use crate::wit::papokin::plugin::event::{Event, EventType, PreFillProfileEventData};

use super::super::FromIntoEvent;

/// 玩家档案填充前触发的事件，允许通过
/// 可通过属性提供完整的 profile。
pub struct PreFillProfileEvent;
impl FromIntoEvent for PreFillProfileEvent {
    const EVENT_TYPE: EventType = EventType::PreFillProfileEvent;
    type Data = PreFillProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreFillProfileEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreFillProfileEvent(data)
    }
}
