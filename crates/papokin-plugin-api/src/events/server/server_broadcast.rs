use crate::wit::papokin::plugin::event::{Event, EventType, ServerBroadcastEventData};

use super::super::FromIntoEvent;

/// 向服务器全体玩家广播消息时触发的事件。
///
/// 关联的 [`ServerBroadcastEventData`] 包含消息和发送者。
/// 此事件可取消。
pub struct ServerBroadcastEvent;
impl FromIntoEvent for ServerBroadcastEvent {
    const EVENT_TYPE: EventType = EventType::ServerBroadcastEvent;
    type Data = ServerBroadcastEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerBroadcastEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerBroadcastEvent(data)
    }
}
