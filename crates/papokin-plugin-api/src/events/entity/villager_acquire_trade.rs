use crate::wit::papokin::plugin::event::{Event, EventType, VillagerAcquireTradeEventData};

use super::super::FromIntoEvent;

/// 村民获得新交易时触发的事件。
pub struct VillagerAcquireTradeEvent;
impl FromIntoEvent for VillagerAcquireTradeEvent {
    const EVENT_TYPE: EventType = EventType::VillagerAcquireTradeEvent;
    type Data = VillagerAcquireTradeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VillagerAcquireTradeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VillagerAcquireTradeEvent(data)
    }
}
