use crate::wit::papokin::plugin::event::{Event, EventType, UncheckedSignChangeEventData};

use super::super::FromIntoEvent;

/// 玩家修改告示牌时触发的事件，在校验
/// 检查会被应用。此事件可取消。
pub struct UncheckedSignChangeEvent;
impl FromIntoEvent for UncheckedSignChangeEvent {
    const EVENT_TYPE: EventType = EventType::UncheckedSignChangeEvent;
    type Data = UncheckedSignChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::UncheckedSignChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::UncheckedSignChangeEvent(data)
    }
}
