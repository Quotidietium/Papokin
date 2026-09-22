use crate::wit::papokin::plugin::event::{Event, EventType, PlayerHideEntityEventData};

use super::super::FromIntoEvent;

/// 实体对玩家隐藏时触发的事件。
pub struct PlayerHideEntityEvent;
impl FromIntoEvent for PlayerHideEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerHideEntityEvent;
    type Data = PlayerHideEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerHideEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerHideEntityEvent(data)
    }
}
