use crate::wit::papokin::plugin::event::{
    AsyncPlayerConnectionConfigureEventData, Event, EventType,
};

use super::super::FromIntoEvent;

/// 配置玩家连接时异步触发的事件。
pub struct AsyncPlayerConnectionConfigureEvent;
impl FromIntoEvent for AsyncPlayerConnectionConfigureEvent {
    const EVENT_TYPE: EventType = EventType::AsyncPlayerConnectionConfigureEvent;
    type Data = AsyncPlayerConnectionConfigureEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncPlayerConnectionConfigureEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncPlayerConnectionConfigureEvent(data)
    }
}
