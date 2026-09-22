use crate::wit::papokin::plugin::event::{Event, EventType, PlayerChangeWorldEventData};

use super::super::FromIntoEvent;

/// 玩家切换世界时触发的事件。
///
/// 关联的 [`PlayerChangeWorldEventData`] 包含玩家、原世界、
/// 新世界，以及目标位置、偏航角和俯仰角。该事件可取消。
pub struct PlayerChangeWorldEvent;
impl FromIntoEvent for PlayerChangeWorldEvent {
    const EVENT_TYPE: EventType = EventType::PlayerChangeWorldEvent;
    type Data = PlayerChangeWorldEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerChangeWorldEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerChangeWorldEvent(data)
    }
}
