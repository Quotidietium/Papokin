use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLoginEventData};

use super::super::FromIntoEvent;

/// 玩家尝试登录服务器时触发的事件。
///
/// 关联的 [`PlayerLoginEventData`] 包含玩家和踢出消息
/// 在登录被取消时使用。此事件可取消。
pub struct PlayerLoginEvent;
impl FromIntoEvent for PlayerLoginEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLoginEvent;
    type Data = PlayerLoginEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLoginEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLoginEvent(data)
    }
}
