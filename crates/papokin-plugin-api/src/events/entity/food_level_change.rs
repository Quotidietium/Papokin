use crate::wit::papokin::plugin::event::{Event, EventType, FoodLevelChangeEventData};

use super::super::FromIntoEvent;

/// 实体饥饿值变化时触发的事件。
pub struct FoodLevelChangeEvent;
impl FromIntoEvent for FoodLevelChangeEvent {
    const EVENT_TYPE: EventType = EventType::FoodLevelChangeEvent;
    type Data = FoodLevelChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FoodLevelChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FoodLevelChangeEvent(data)
    }
}
