use crate::wit::papokin::plugin::event::{Event, EventType, PlayerJoinEventData};

use super::super::FromIntoEvent;

/// 玩家加入服务器时触发的事件。
///
/// 关联的 [`PlayerJoinEventData`] 包含玩家和加入消息
/// 可被修改或抑制的内容。该事件可取消。
pub struct PlayerJoinEvent;
impl FromIntoEvent for PlayerJoinEvent {
    const EVENT_TYPE: EventType = EventType::PlayerJoinEvent;
    type Data = PlayerJoinEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerJoinEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerJoinEvent(data)
    }
}
