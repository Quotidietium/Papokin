use crate::wit::papokin::plugin::event::{Event, EventType, ProfileWhitelistVerifyEventData};

use super::super::FromIntoEvent;

/// 连接中的玩家按白名单验证时触发的事件。
pub struct ProfileWhitelistVerifyEvent;
impl FromIntoEvent for ProfileWhitelistVerifyEvent {
    const EVENT_TYPE: EventType = EventType::ProfileWhitelistVerifyEvent;
    type Data = ProfileWhitelistVerifyEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ProfileWhitelistVerifyEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ProfileWhitelistVerifyEvent(data)
    }
}
