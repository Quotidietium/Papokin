use crate::wit::papokin::plugin::event::{Event, EventType, PlayerExpChangeEventData};

use super::super::FromIntoEvent;

/// 玩家经验变化时触发的事件。
///
/// 关联的 [`PlayerExpChangeEventData`] 包含玩家以及经验的
/// 要增加的经验值（可以为负）。
pub struct PlayerExpChangeEvent;
impl FromIntoEvent for PlayerExpChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerExpChangeEvent;
    type Data = PlayerExpChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerExpChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerExpChangeEvent(data)
    }
}
