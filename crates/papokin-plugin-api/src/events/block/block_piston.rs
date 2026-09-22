use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{
    BlockPistonExtendEventData, BlockPistonRetractEventData, Event, EventType,
};

/// 活塞推出时触发的事件。
pub struct BlockPistonExtendEvent;
impl FromIntoEvent for BlockPistonExtendEvent {
    const EVENT_TYPE: EventType = EventType::BlockPistonExtendEvent;
    type Data = BlockPistonExtendEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockPistonExtendEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockPistonExtendEvent(data)
    }
}

/// 活塞收回时触发的事件。
pub struct BlockPistonRetractEvent;
impl FromIntoEvent for BlockPistonRetractEvent {
    const EVENT_TYPE: EventType = EventType::BlockPistonRetractEvent;
    type Data = BlockPistonRetractEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockPistonRetractEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockPistonRetractEvent(data)
    }
}
