use crate::wit::papokin::plugin::event::{Event, EventType, TameableDeathMessageEventData};

use super::super::FromIntoEvent;

/// 可驯服实体死亡并生成其死亡消息时触发的事件。
pub struct TameableDeathMessageEvent;
impl FromIntoEvent for TameableDeathMessageEvent {
    const EVENT_TYPE: EventType = EventType::TameableDeathMessageEvent;
    type Data = TameableDeathMessageEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::TameableDeathMessageEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::TameableDeathMessageEvent(data)
    }
}
