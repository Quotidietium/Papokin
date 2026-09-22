use crate::wit::papokin::plugin::event::{Event, EventType, LingeringPotionSplashEventData};

use super::super::FromIntoEvent;

/// 滞留药水溅射时触发的事件。
pub struct LingeringPotionSplashEvent;
impl FromIntoEvent for LingeringPotionSplashEvent {
    const EVENT_TYPE: EventType = EventType::LingeringPotionSplashEvent;
    type Data = LingeringPotionSplashEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::LingeringPotionSplashEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::LingeringPotionSplashEvent(data)
    }
}
