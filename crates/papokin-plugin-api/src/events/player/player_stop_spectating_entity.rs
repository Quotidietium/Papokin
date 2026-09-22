use crate::wit::papokin::plugin::event::{Event, EventType, PlayerStopSpectatingEntityEventData};

use super::super::FromIntoEvent;

/// 玩家停止旁观实体时触发的事件。此事件
/// 此事件可取消。
pub struct PlayerStopSpectatingEntityEvent;
impl FromIntoEvent for PlayerStopSpectatingEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStopSpectatingEntityEvent;
    type Data = PlayerStopSpectatingEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStopSpectatingEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStopSpectatingEntityEvent(data)
    }
}
