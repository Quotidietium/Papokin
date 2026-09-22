use crate::wit::papokin::plugin::event::{BlockFertilizeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块被施肥时触发的事件。
pub struct BlockFertilizeEvent;
impl FromIntoEvent for BlockFertilizeEvent {
    const EVENT_TYPE: EventType = EventType::BlockFertilizeEvent;
    type Data = BlockFertilizeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockFertilizeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockFertilizeEvent(data)
    }
}
