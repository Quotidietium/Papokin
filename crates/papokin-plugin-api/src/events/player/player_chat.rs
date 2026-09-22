use crate::wit::papokin::plugin::event::{Event, EventType, PlayerChatEventData};

use super::super::FromIntoEvent;

/// 玩家发送聊天消息时触发的事件。
///
/// 关联的 [`PlayerChatEventData`] 包含玩家、消息以及
/// 接收者列表。消息可以被修改。该事件可取消。
pub struct PlayerChatEvent;
impl FromIntoEvent for PlayerChatEvent {
    const EVENT_TYPE: EventType = EventType::PlayerChatEvent;
    type Data = PlayerChatEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerChatEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerChatEvent(data)
    }
}
