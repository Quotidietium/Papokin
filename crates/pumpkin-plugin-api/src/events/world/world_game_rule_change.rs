use crate::wit::pumpkin::plugin::event::{Event, EventType, WorldGameRuleChangeEventData};

use super::super::FromIntoEvent;

/// Event triggered when a game rule value changes in a world.
pub struct WorldGameRuleChangeEvent;
impl FromIntoEvent for WorldGameRuleChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldGameRuleChangeEvent;
    type Data = WorldGameRuleChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldGameRuleChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldGameRuleChangeEvent(data)
    }
}
