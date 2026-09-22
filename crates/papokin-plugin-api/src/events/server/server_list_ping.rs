use crate::wit::papokin::plugin::event::{Event, EventType, ServerListPingEventData};

use super::super::FromIntoEvent;

/// 服务器准备 Java 版状态/列表 ping 响应时触发。
///
/// 将其注册为阻塞式事件处理器，以自定义 MOTD、favicon、
/// 以及为 WASM 插件报告玩家数量。
pub struct ServerListPingEvent;

impl FromIntoEvent for ServerListPingEvent {
    const EVENT_TYPE: EventType = EventType::ServerListPingEvent;
    type Data = ServerListPingEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerListPingEvent(data) => data,
            _ => panic!("预期为 ServerListPingEvent"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerListPingEvent(data)
    }
}
