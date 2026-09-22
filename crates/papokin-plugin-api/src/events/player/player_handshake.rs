use crate::wit::papokin::plugin::event::{Event, EventType, PlayerHandshakeEventData};

use super::super::FromIntoEvent;

/// 客户端发送握手包时触发的事件。此事件
/// 可取消；取消会立即断开客户端的连接。
pub struct PlayerHandshakeEvent;
impl FromIntoEvent for PlayerHandshakeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerHandshakeEvent;
    type Data = PlayerHandshakeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerHandshakeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerHandshakeEvent(data)
    }
}
