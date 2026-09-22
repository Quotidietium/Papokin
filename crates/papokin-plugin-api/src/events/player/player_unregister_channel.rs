use crate::wit::papokin::plugin::event::{Event, EventType, PlayerUnregisterChannelEventData};

use super::super::FromIntoEvent;

/// 为玩家注销插件通道时触发的事件。
pub struct PlayerUnregisterChannelEvent;
impl FromIntoEvent for PlayerUnregisterChannelEvent {
    const EVENT_TYPE: EventType = EventType::PlayerUnregisterChannelEvent;
    type Data = PlayerUnregisterChannelEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerUnregisterChannelEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerUnregisterChannelEvent(data)
    }
}
