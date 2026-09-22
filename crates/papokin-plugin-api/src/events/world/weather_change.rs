use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{
    Event, EventType, ThunderChangeEventData, WeatherChangeEventData,
};

/// 世界天气变化时触发的事件。
pub struct WeatherChangeEvent;
impl FromIntoEvent for WeatherChangeEvent {
    const EVENT_TYPE: EventType = EventType::WeatherChangeEvent;
    type Data = WeatherChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WeatherChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WeatherChangeEvent(data)
    }
}

/// 雷暴状态变化时触发的事件。
pub struct ThunderChangeEvent;
impl FromIntoEvent for ThunderChangeEvent {
    const EVENT_TYPE: EventType = EventType::ThunderChangeEvent;
    type Data = ThunderChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ThunderChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ThunderChangeEvent(data)
    }
}
