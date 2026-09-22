use crate::wit::papokin::plugin::event::{DialogClickActionEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家点击自定义对话框按钮时触发的事件。
pub struct DialogClickActionEvent;

impl FromIntoEvent for DialogClickActionEvent {
    const EVENT_TYPE: EventType = EventType::DialogClickActionEvent;
    type Data = DialogClickActionEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::DialogClickActionEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::DialogClickActionEvent(data)
    }
}
