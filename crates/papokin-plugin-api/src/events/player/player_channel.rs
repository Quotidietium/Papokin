use crate::wit::papokin::plugin::event::{Event, EventType, PlayerChannelEventData};

use super::super::FromIntoEvent;

/// 玩家切换插件消息通道时触发的事件。
pub struct PlayerChannelEvent;
impl FromIntoEvent for PlayerChannelEvent {
    const EVENT_TYPE: EventType = EventType::PlayerChannelEvent;
    type Data = PlayerChannelEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerChannelEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerChannelEvent(data)
    }
}
