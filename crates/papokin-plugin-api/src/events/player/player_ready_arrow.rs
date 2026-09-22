use crate::wit::papokin::plugin::event::{Event, EventType, PlayerReadyArrowEventData};

use super::super::FromIntoEvent;

/// 玩家用弓搭箭时触发的事件。此事件
/// 此事件可取消。
pub struct PlayerReadyArrowEvent;
impl FromIntoEvent for PlayerReadyArrowEvent {
    const EVENT_TYPE: EventType = EventType::PlayerReadyArrowEvent;
    type Data = PlayerReadyArrowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerReadyArrowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerReadyArrowEvent(data)
    }
}
