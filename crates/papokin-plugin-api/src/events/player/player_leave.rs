use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLeaveEventData};

use super::super::FromIntoEvent;

/// 玩家离开服务器时触发的事件。
///
/// 关联的 [`PlayerLeaveEventData`] 包含玩家和离开消息
/// 可被修改或抑制的内容。该事件可取消。
pub struct PlayerLeaveEvent;
impl FromIntoEvent for PlayerLeaveEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLeaveEvent;
    type Data = PlayerLeaveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLeaveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLeaveEvent(data)
    }
}
