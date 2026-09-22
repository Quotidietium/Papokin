use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLoomPatternSelectEventData};

use super::super::FromIntoEvent;

/// 玩家在织布机中选择图案时触发的事件。此事件
/// 此事件可取消。
pub struct PlayerLoomPatternSelectEvent;
impl FromIntoEvent for PlayerLoomPatternSelectEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLoomPatternSelectEvent;
    type Data = PlayerLoomPatternSelectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLoomPatternSelectEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLoomPatternSelectEvent(data)
    }
}
