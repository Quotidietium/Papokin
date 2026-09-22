use crate::wit::papokin::plugin::event::{Event, EventType, WhitelistStateUpdateEventData};

use super::super::FromIntoEvent;

/// 玩家白名单状态更新时触发的事件。
pub struct WhitelistStateUpdateEvent;
impl FromIntoEvent for WhitelistStateUpdateEvent {
    const EVENT_TYPE: EventType = EventType::WhitelistStateUpdateEvent;
    type Data = WhitelistStateUpdateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WhitelistStateUpdateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WhitelistStateUpdateEvent(data)
    }
}
