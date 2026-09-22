use crate::wit::papokin::plugin::event::{Event, EventType, WhitelistToggleEventData};

use super::super::FromIntoEvent;

/// 白名单开关切换时触发的事件。
pub struct WhitelistToggleEvent;
impl FromIntoEvent for WhitelistToggleEvent {
    const EVENT_TYPE: EventType = EventType::WhitelistToggleEvent;
    type Data = WhitelistToggleEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WhitelistToggleEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WhitelistToggleEvent(data)
    }
}
