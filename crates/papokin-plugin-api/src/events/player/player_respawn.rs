use crate::wit::papokin::plugin::event::{Event, EventType, PlayerRespawnEventData};

use super::super::FromIntoEvent;

/// 玩家重生时触发的事件。
///
/// 关联的 [`PlayerRespawnEventData`] 包含玩家、其
/// 重生前的世界、重生后进入的世界，以及目标位置，
/// 偏航角与俯仰角。此事件不可取消。
pub struct PlayerRespawnEvent;
impl FromIntoEvent for PlayerRespawnEvent {
    const EVENT_TYPE: EventType = EventType::PlayerRespawnEvent;
    type Data = PlayerRespawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerRespawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerRespawnEvent(data)
    }
}
