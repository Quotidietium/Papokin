use crate::wit::papokin::plugin::event::{Event, EventType, VillagerReplenishTradeEventData};

use super::super::FromIntoEvent;

/// 村民补充交易时触发的事件。
pub struct VillagerReplenishTradeEvent;
impl FromIntoEvent for VillagerReplenishTradeEvent {
    const EVENT_TYPE: EventType = EventType::VillagerReplenishTradeEvent;
    type Data = VillagerReplenishTradeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VillagerReplenishTradeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VillagerReplenishTradeEvent(data)
    }
}
