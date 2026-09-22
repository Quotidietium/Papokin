use crate::wit::papokin::plugin::event::{BlockDispenseLootEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块发放战利品时触发的事件。
pub struct BlockDispenseLootEvent;
impl FromIntoEvent for BlockDispenseLootEvent {
    const EVENT_TYPE: EventType = EventType::BlockDispenseLootEvent;
    type Data = BlockDispenseLootEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockDispenseLootEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockDispenseLootEvent(data)
    }
}
