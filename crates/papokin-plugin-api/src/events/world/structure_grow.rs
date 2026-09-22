use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, StructureGrowEventData};

/// 结构生长时触发的事件。
pub struct StructureGrowEvent;
impl FromIntoEvent for StructureGrowEvent {
    const EVENT_TYPE: EventType = EventType::StructureGrowEvent;
    type Data = StructureGrowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::StructureGrowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::StructureGrowEvent(data)
    }
}
