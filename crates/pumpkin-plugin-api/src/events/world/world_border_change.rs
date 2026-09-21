use crate::wit::pumpkin::plugin::event::{
    Event, EventType, WorldBorderBoundsChangeEventData, WorldBorderCenterChangeEventData,
};

use super::super::FromIntoEvent;

/// Event triggered when the bounds (diameter) of a world border change.
pub struct WorldBorderBoundsChangeEvent;
impl FromIntoEvent for WorldBorderBoundsChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldBorderBoundsChangeEvent;
    type Data = WorldBorderBoundsChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldBorderBoundsChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldBorderBoundsChangeEvent(data)
    }
}

/// Event triggered when the center of a world border changes.
pub struct WorldBorderCenterChangeEvent;
impl FromIntoEvent for WorldBorderCenterChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldBorderCenterChangeEvent;
    type Data = WorldBorderCenterChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldBorderCenterChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldBorderCenterChangeEvent(data)
    }
}
