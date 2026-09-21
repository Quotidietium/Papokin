use crate::wit::pumpkin::plugin::event::{
    Event, EventType, PlayerNaturallySpawnCreaturesEventData,
};

use super::super::FromIntoEvent;

/// An event that occurs when creatures are naturally spawned around a player.
/// This event is cancellable.
pub struct PlayerNaturallySpawnCreaturesEvent;
impl FromIntoEvent for PlayerNaturallySpawnCreaturesEvent {
    const EVENT_TYPE: EventType = EventType::PlayerNaturallySpawnCreaturesEvent;
    type Data = PlayerNaturallySpawnCreaturesEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerNaturallySpawnCreaturesEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerNaturallySpawnCreaturesEvent(data)
    }
}
