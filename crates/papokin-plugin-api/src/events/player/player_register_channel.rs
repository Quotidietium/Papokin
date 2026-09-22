use crate::wit::papokin::plugin::event::{Event, EventType, PlayerRegisterChannelEventData};

use super::super::FromIntoEvent;

/// 为玩家注册插件通道时触发的事件。
pub struct PlayerRegisterChannelEvent;
impl FromIntoEvent for PlayerRegisterChannelEvent {
    const EVENT_TYPE: EventType = EventType::PlayerRegisterChannelEvent;
    type Data = PlayerRegisterChannelEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerRegisterChannelEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerRegisterChannelEvent(data)
    }
}
