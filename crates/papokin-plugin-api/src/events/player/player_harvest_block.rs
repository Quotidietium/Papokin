use crate::wit::papokin::plugin::event::{Event, EventType, PlayerHarvestBlockEventData};

use super::super::FromIntoEvent;

/// 玩家收获方块时触发的事件。
pub struct PlayerHarvestBlockEvent;
impl FromIntoEvent for PlayerHarvestBlockEvent {
    const EVENT_TYPE: EventType = EventType::PlayerHarvestBlockEvent;
    type Data = PlayerHarvestBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerHarvestBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerHarvestBlockEvent(data)
    }
}
