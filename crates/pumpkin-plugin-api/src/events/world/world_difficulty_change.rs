use crate::wit::pumpkin::plugin::event::{Event, EventType, WorldDifficultyChangeEventData};

use super::super::FromIntoEvent;

/// Event triggered when the difficulty of a world changes.
pub struct WorldDifficultyChangeEvent;
impl FromIntoEvent for WorldDifficultyChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldDifficultyChangeEvent;
    type Data = WorldDifficultyChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldDifficultyChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldDifficultyChangeEvent(data)
    }
}
