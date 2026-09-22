use crate::wit::papokin::plugin::event::{
    Event, EventType, WorldBorderBoundsChangeEventData, WorldBorderCenterChangeEventData,
};

use super::super::FromIntoEvent;

/// 世界边界范围（直径）变化时触发的事件。
pub struct WorldBorderBoundsChangeEvent;
impl FromIntoEvent for WorldBorderBoundsChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldBorderBoundsChangeEvent;
    type Data = WorldBorderBoundsChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldBorderBoundsChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldBorderBoundsChangeEvent(data)
    }
}

/// 世界边界中心变化时触发的事件。
pub struct WorldBorderCenterChangeEvent;
impl FromIntoEvent for WorldBorderCenterChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldBorderCenterChangeEvent;
    type Data = WorldBorderCenterChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldBorderCenterChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldBorderCenterChangeEvent(data)
    }
}
