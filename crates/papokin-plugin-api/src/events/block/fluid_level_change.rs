use crate::wit::papokin::plugin::event::{Event, EventType, FluidLevelChangeEventData};

use super::super::FromIntoEvent;

/// 流体液位变化时触发的事件。
pub struct FluidLevelChangeEvent;
impl FromIntoEvent for FluidLevelChangeEvent {
    const EVENT_TYPE: EventType = EventType::FluidLevelChangeEvent;
    type Data = FluidLevelChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FluidLevelChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FluidLevelChangeEvent(data)
    }
}
