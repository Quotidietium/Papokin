use super::World;
use papokin_protocol::java::client::play::{CGameEvent, GameEvent};
use rand::RngExt;

// 天气计时常量
const RAIN_DELAY_MIN: i32 = 12_000;
const RAIN_DELAY_MAX: i32 = 180_000;
const RAIN_DURATION_MIN: i32 = 12_000;
const RAIN_DURATION_MAX: i32 = 24_000;
const THUNDER_DELAY_MIN: i32 = 12_000;
const THUNDER_DELAY_MAX: i32 = 180_000;
const THUNDER_DURATION_MIN: i32 = 3_600;
const THUNDER_DURATION_MAX: i32 = 15_600;

const WEATHER_TRANSITION_SPEED: f32 = 0.01;

pub struct Weather {
    pub clear_weather_time: i32,
    pub raining: bool,
    pub rain_time: i32,
    pub thundering: bool,
    pub thunder_time: i32,

    pub rain_level: f32,
    pub old_rain_level: f32,
    pub thunder_level: f32,
    pub old_thunder_level: f32,

    pub weather_cycle_enabled: bool,
}

impl Default for Weather {
    fn default() -> Self {
        Self::new()
    }
}

impl Weather {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            clear_weather_time: 0,
            raining: false,
            rain_time: 0,
            thundering: false,
            thunder_time: 0,
            rain_level: 0.0,
            old_rain_level: 0.0,
            thunder_level: 0.0,
            old_thunder_level: 0.0,
            weather_cycle_enabled: true,
        }
    }

    pub fn set_weather_parameters(
        &mut self,
        world: &World,
        clear_time: i32,
        rain_time: i32,
        raining: bool,
        thundering: bool,
    ) {
        let was_raining = self.raining;

        self.clear_weather_time = clear_time;
        self.rain_time = rain_time;
        self.thunder_time = rain_time;
        self.raining = raining;
        self.thundering = thundering;

        if was_raining != raining {
            if was_raining {
                world.broadcast_packet_all(&CGameEvent::new(GameEvent::EndRaining, 0.0));
            } else {
                world.broadcast_packet_all(&CGameEvent::new(GameEvent::BeginRaining, 0.0));
            }
        }
    }

    pub fn tick_weather(&mut self, world: &World) {
        // 与 gamerule `advance_weather`（原版 doWeatherCycle）实时同步：
        // 该规则变更应立即生效，循环仅在启用时推进。
        self.weather_cycle_enabled = world.level_info.load().game_rules.advance_weather;
        if self.weather_cycle_enabled {
            self.advance_weather_cycle();
        }

        // 更新视觉过渡
        self.old_rain_level = self.rain_level;
        self.old_thunder_level = self.thunder_level;

        if self.raining {
            self.rain_level = (self.rain_level + WEATHER_TRANSITION_SPEED).min(1.0);
        } else {
            self.rain_level = (self.rain_level - WEATHER_TRANSITION_SPEED).max(0.0);
        }

        if self.thundering {
            self.thunder_level = (self.thunder_level + WEATHER_TRANSITION_SPEED).min(1.0);
        } else {
            self.thunder_level = (self.thunder_level - WEATHER_TRANSITION_SPEED).max(0.0);
        }

        // 如有需要，广播等级变化
        if (self.old_rain_level - self.rain_level).abs() > f32::EPSILON {
            world.broadcast_packet_all(&CGameEvent::new(
                GameEvent::RainLevelChange,
                self.rain_level,
            ));
        }

        if (self.old_thunder_level - self.thunder_level).abs() > f32::EPSILON {
            world.broadcast_packet_all(&CGameEvent::new(
                GameEvent::ThunderLevelChange,
                self.thunder_level,
            ));
        }
    }

    fn advance_weather_cycle(&mut self) {
        // 由于没有 await 调用，移除了 async
        if self.clear_weather_time > 0 {
            self.clear_weather_time -= 1;
            self.thunder_time = i32::from(!self.thundering);
            self.rain_time = i32::from(!self.raining);
            self.thundering = false;
            self.raining = false;
        } else {
            // 处理雷暴计时
            if self.thunder_time > 0 {
                self.thunder_time -= 1;
                if self.thunder_time == 0 {
                    self.thundering = !self.thundering;
                }
            } else if self.thundering {
                self.thunder_time =
                    rand::rng().random_range(THUNDER_DURATION_MIN..=THUNDER_DURATION_MAX);
            } else {
                self.thunder_time = rand::rng().random_range(THUNDER_DELAY_MIN..=THUNDER_DELAY_MAX);
            }

            // 处理降雨计时
            if self.rain_time > 0 {
                self.rain_time -= 1;
                if self.rain_time == 0 {
                    self.raining = !self.raining;
                }
            } else if self.raining {
                self.rain_time = rand::rng().random_range(RAIN_DURATION_MIN..=RAIN_DURATION_MAX);
            } else {
                self.rain_time = rand::rng().random_range(RAIN_DELAY_MIN..=RAIN_DELAY_MAX);
            }
        }
    }

    pub fn reset_weather_cycle(&mut self, world: &World) {
        self.set_weather_parameters(world, 0, 0, false, false);
    }
}

impl Clone for Weather {
    fn clone(&self) -> Self {
        Self {
            clear_weather_time: self.clear_weather_time,
            raining: self.raining,
            rain_time: self.rain_time,
            thundering: self.thundering,
            thunder_time: self.thunder_time,
            rain_level: self.rain_level,
            old_rain_level: self.old_rain_level,
            thunder_level: self.thunder_level,
            old_thunder_level: self.old_thunder_level,
            weather_cycle_enabled: self.weather_cycle_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 晴天计时应逐刻递减，且期间强制保持无雨无雷状态。
    #[test]
    fn clear_weather_time_counts_down_and_forces_clear() {
        let mut weather = Weather::new();
        weather.clear_weather_time = 100;

        weather.advance_weather_cycle();

        assert_eq!(weather.clear_weather_time, 99);
        assert!(!weather.raining);
        assert!(!weather.thundering);
    }

    /// 雨天计时归零时翻转下雨状态；重新随机的新计时在下一刻才产生。
    #[test]
    fn rain_time_expiring_toggles_raining() {
        let mut weather = Weather::new();
        weather.raining = true;
        weather.rain_time = 1;

        weather.advance_weather_cycle();

        assert!(!weather.raining);
        assert_eq!(weather.rain_time, 0);

        // 翻转后的下一刻进入无雨延迟计时
        weather.advance_weather_cycle();
        assert!(weather.rain_time >= RAIN_DELAY_MIN && weather.rain_time <= RAIN_DELAY_MAX);
    }

    /// 雨量过渡在 0..=1 内饱和，不会越界。
    #[test]
    fn rain_level_saturates_within_bounds() {
        let mut weather = Weather::new();
        weather.rain_level = 0.999;

        for _ in 0..4 {
            weather.rain_level = (weather.rain_level + WEATHER_TRANSITION_SPEED).min(1.0);
        }

        assert_eq!(weather.rain_level, 1.0);
    }
}
