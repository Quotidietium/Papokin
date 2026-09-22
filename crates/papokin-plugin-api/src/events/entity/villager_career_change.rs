use crate::wit::papokin::plugin::event::{Event, EventType, VillagerCareerChangeEventData};

use super::super::FromIntoEvent;

/// 村民更换职业时触发的事件。
pub struct VillagerCareerChangeEvent;
impl FromIntoEvent for VillagerCareerChangeEvent {
    const EVENT_TYPE: EventType = EventType::VillagerCareerChangeEvent;
    type Data = VillagerCareerChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VillagerCareerChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VillagerCareerChangeEvent(data)
    }
}
