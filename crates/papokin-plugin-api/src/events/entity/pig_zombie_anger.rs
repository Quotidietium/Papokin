use crate::wit::papokin::plugin::event::{Event, EventType, PigZombieAngerEventData};

use super::super::FromIntoEvent;

/// 僵尸猪人发怒时触发的事件。
pub struct PigZombieAngerEvent;
impl FromIntoEvent for PigZombieAngerEvent {
    const EVENT_TYPE: EventType = EventType::PigZombieAngerEvent;
    type Data = PigZombieAngerEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PigZombieAngerEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PigZombieAngerEvent(data)
    }
}
