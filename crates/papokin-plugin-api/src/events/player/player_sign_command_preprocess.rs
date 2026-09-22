use crate::wit::papokin::plugin::event::{Event, EventType, PlayerSignCommandPreprocessEventData};

use super::super::FromIntoEvent;

/// 告示牌上的命令在
/// 执行。此事件可取消；命令可被修改。
pub struct PlayerSignCommandPreprocessEvent;
impl FromIntoEvent for PlayerSignCommandPreprocessEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSignCommandPreprocessEvent;
    type Data = PlayerSignCommandPreprocessEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSignCommandPreprocessEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSignCommandPreprocessEvent(data)
    }
}
