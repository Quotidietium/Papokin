use crate::wit::papokin::plugin::event::{Event, EventType, PlayerCommandSendEventData};

use super::super::FromIntoEvent;

/// 玩家发送命令时触发的事件。
///
/// 关联的 [`PlayerCommandSendEventData`] 包含玩家和命令
/// 字符串（不含开头的 `/`）。该事件可取消。
pub struct PlayerCommandSendEvent;
impl FromIntoEvent for PlayerCommandSendEvent {
    const EVENT_TYPE: EventType = EventType::PlayerCommandSendEvent;
    type Data = PlayerCommandSendEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerCommandSendEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerCommandSendEvent(data)
    }
}
