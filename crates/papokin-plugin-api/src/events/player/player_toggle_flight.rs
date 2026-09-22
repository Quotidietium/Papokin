use crate::wit::papokin::plugin::event::{Event, EventType, PlayerToggleFlightEventData};

use super::super::FromIntoEvent;

/// 玩家切换飞行时触发的事件。
pub struct PlayerToggleFlightEvent;
impl FromIntoEvent for PlayerToggleFlightEvent {
    const EVENT_TYPE: EventType = EventType::PlayerToggleFlightEvent;
    type Data = PlayerToggleFlightEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerToggleFlightEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerToggleFlightEvent(data)
    }
}
