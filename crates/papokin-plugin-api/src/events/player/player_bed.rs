use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerBedEnterEventData, PlayerBedLeaveEventData,
};

/// 玩家上床时触发的事件。
pub struct PlayerBedEnterEvent;
impl FromIntoEvent for PlayerBedEnterEvent {
    const EVENT_TYPE: EventType = EventType::PlayerBedEnterEvent;
    type Data = PlayerBedEnterEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerBedEnterEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerBedEnterEvent(data)
    }
}

/// 玩家下床时触发的事件。
pub struct PlayerBedLeaveEvent;
impl FromIntoEvent for PlayerBedLeaveEvent {
    const EVENT_TYPE: EventType = EventType::PlayerBedLeaveEvent;
    type Data = PlayerBedLeaveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerBedLeaveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerBedLeaveEvent(data)
    }
}
