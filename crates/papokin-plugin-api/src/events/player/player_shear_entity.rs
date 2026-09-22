use crate::wit::papokin::plugin::event::{Event, EventType, PlayerShearEntityEventData};

use super::super::FromIntoEvent;

/// 玩家剪实体时触发的事件。
pub struct PlayerShearEntityEvent;
impl FromIntoEvent for PlayerShearEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerShearEntityEvent;
    type Data = PlayerShearEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerShearEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerShearEntityEvent(data)
    }
}
