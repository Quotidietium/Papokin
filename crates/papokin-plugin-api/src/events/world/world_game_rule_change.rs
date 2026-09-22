use crate::wit::papokin::plugin::event::{Event, EventType, WorldGameRuleChangeEventData};

use super::super::FromIntoEvent;

/// 世界中游戏规则值变化时触发的事件。
pub struct WorldGameRuleChangeEvent;
impl FromIntoEvent for WorldGameRuleChangeEvent {
    const EVENT_TYPE: EventType = EventType::WorldGameRuleChangeEvent;
    type Data = WorldGameRuleChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WorldGameRuleChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WorldGameRuleChangeEvent(data)
    }
}
