use crate::wit::papokin::plugin::event::{Event, EventType, PlayerStartSpectatingEntityEventData};

use super::super::FromIntoEvent;

/// 玩家开始旁观实体时触发的事件。此事件
/// 此事件可取消。
pub struct PlayerStartSpectatingEntityEvent;
impl FromIntoEvent for PlayerStartSpectatingEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStartSpectatingEntityEvent;
    type Data = PlayerStartSpectatingEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStartSpectatingEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStartSpectatingEntityEvent(data)
    }
}
