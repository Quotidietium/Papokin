use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPickupArrowEventData};

use super::super::FromIntoEvent;

/// 玩家拾起箭时触发的事件。
pub struct PlayerPickupArrowEvent;
impl FromIntoEvent for PlayerPickupArrowEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPickupArrowEvent;
    type Data = PlayerPickupArrowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPickupArrowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPickupArrowEvent(data)
    }
}
