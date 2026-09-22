use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInteractUnknownEntityEventData};

use super::super::FromIntoEvent;

/// 玩家与服务器无法解析的实体 ID 交互时触发的事件。
///
/// 关联的 [`PlayerInteractUnknownEntityEventData`] 包含玩家、未知的
/// 实体 ID，以及尝试进行的交互动作。此事件可取消。
pub struct PlayerInteractUnknownEntityEvent;

impl FromIntoEvent for PlayerInteractUnknownEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInteractUnknownEntityEvent;
    type Data = PlayerInteractUnknownEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInteractUnknownEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInteractUnknownEntityEvent(data)
    }
}
