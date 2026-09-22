use crate::wit::papokin::plugin::event::{Event, EventType, VillagerReputationChangeEventData};

use super::super::FromIntoEvent;

/// 村民声望变化时触发的事件。
pub struct VillagerReputationChangeEvent;
impl FromIntoEvent for VillagerReputationChangeEvent {
    const EVENT_TYPE: EventType = EventType::VillagerReputationChangeEvent;
    type Data = VillagerReputationChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VillagerReputationChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VillagerReputationChangeEvent(data)
    }
}
