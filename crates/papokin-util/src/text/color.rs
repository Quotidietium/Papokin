use colored::{ColoredString, Colorize};
use serde::{Deserialize, Deserializer, Serialize};

/// 聊天组件的文本颜色。
///
/// 颜色可以通过三种方式指定：
/// - `Reset` - 使用当前上下文的默认颜色
/// - `Rgb` - 自定义 RGB 颜色（例如 "#FF55AA"）
/// - `Named` - 16 种标准 Minecraft 命名颜色之一
#[derive(Default, Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum Color {
    /// 将使用文本的默认颜色，具体因上下文而异
    /// (某些情况下是白色，某些情况下是黑色，还有些情况下
    /// 是一种通常不用于文本的灰色）。
    #[default]
    Reset,
    /// 一种 RGB 颜色，以 "#RRGGBB" 这样的十六进制字符串指定。
    Rgb(RGBColor),
    /// 16 种 Minecraft 命名颜色之一。
    Named(NamedColor),
}

/// 将 HSV（色相、饱和度、明度）颜色值转换为 RGB。
///
/// # Arguments
/// - `h` – 色相，以度为单位（0-360）
/// - `s` – 饱和度，浮点数（0-1）
/// - `v` – 数值（亮度），浮点数（0-1）
///
/// # Returns
/// 一个（红、绿、蓝）元组，以 u8 值（0-255）表示。
#[must_use]
#[expect(clippy::many_single_char_names)]
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h as i32 / 60) % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;

        if s == "reset" {
            Ok(Self::Reset)
        } else if let Some(hex) = s.strip_prefix('#') {
            if s.len() != 7 {
                return Err(serde::de::Error::custom(
                    "Hex color must be in the format '#RRGGBB'",
                ));
            }

            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| serde::de::Error::custom("Invalid red component in hex color"))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| serde::de::Error::custom("Invalid green component in hex color"))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| serde::de::Error::custom("Invalid blue component in hex color"))?;

            Ok(Self::Rgb(RGBColor::new(r, g, b)))
        } else {
            Ok(Self::Named(NamedColor::try_from(s).map_err(|()| {
                serde::de::Error::custom("Invalid named color")
            })?))
        }
    }
}

impl Color {
    /// 将此颜色转换为带颜色的字符串，用于终端输出。
    ///
    /// # Arguments
    /// - `text` – 要着色的文本。
    ///
    /// # Returns
    /// 一个可打印到终端的 `ColoredString`。
    #[must_use]
    pub fn console_color(&self, text: &str) -> ColoredString {
        match self {
            Self::Reset => text.clear(),
            Self::Named(color) => match color {
                NamedColor::Black => text.black(),
                NamedColor::DarkBlue => text.blue(),
                NamedColor::DarkGreen => text.green(),
                NamedColor::DarkAqua => text.cyan(),
                NamedColor::DarkRed => text.red(),
                NamedColor::DarkPurple => text.purple(),
                NamedColor::Gold => text.yellow(),
                NamedColor::Gray | NamedColor::DarkGray => text.bright_black(), // ?
                NamedColor::Blue => text.bright_blue(),
                NamedColor::Green => text.bright_green(),
                NamedColor::Aqua => text.bright_cyan(),
                NamedColor::Red => text.bright_red(),
                NamedColor::LightPurple => text.bright_purple(),
                NamedColor::Yellow => text.bright_yellow(),
                NamedColor::White => text.white(),
            },
            // TODO: 检查终端是否支持真彩色
            Self::Rgb(color) => text.truecolor(color.red, color.green, color.blue),
        }
    }

    /// 根据旧版 Minecraft 颜色代码创建颜色。
    ///
    /// # Arguments
    /// - `code` – 旧版颜色代码字符（0-9、a-f）。
    ///
    /// # Returns
    /// 对应的 `Color`，如果代码无效则为 `None`。
    #[must_use]
    pub const fn from_legacy_code(code: char) -> Option<Self> {
        let named = match code.to_ascii_lowercase() {
            '0' => NamedColor::Black,
            '1' => NamedColor::DarkBlue,
            '2' => NamedColor::DarkGreen,
            '3' => NamedColor::DarkAqua,
            '4' => NamedColor::DarkRed,
            '5' => NamedColor::DarkPurple,
            '6' => NamedColor::Gold,
            '7' => NamedColor::Gray,
            '8' => NamedColor::DarkGray,
            '9' => NamedColor::Blue,
            'a' => NamedColor::Green,
            'b' => NamedColor::Aqua,
            'c' => NamedColor::Red,
            'd' => NamedColor::LightPurple,
            'e' => NamedColor::Yellow,
            'f' => NamedColor::White,
            _ => return None,
        };
        Some(Self::Named(named))
    }

    /// 从十六进制字符串创建 RGB 颜色。
    ///
    /// # Arguments
    /// - `hex` – 不带 '#' 前缀的十六进制颜色字符串，恰好 6 个字符（RRGGBB）。
    ///
    /// # Returns
    /// RGB 颜色；若十六进制字符串无效则为 `None`。
    #[must_use]
    pub fn from_hex_str(hex: &str) -> Option<Self> {
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self::Rgb(RGBColor::new(r, g, b)))
    }

    /// 将自定义 RGB 颜色降采样为最接近的 Minecraft 命名颜色。
    #[must_use]
    pub fn downsample(&self) -> Self {
        match self {
            Self::Rgb(rgb) => Self::Named(rgb.to_nearest_named()),
            other => *other,
        }
    }
}

/// 一种 RGB 颜色，包含红、绿、蓝分量。
#[derive(Debug, Deserialize, Clone, Copy, Eq, Hash, PartialEq)]
pub struct RGBColor {
    /// 红色分量（0-255）。
    pub red: u8,
    /// 绿色分量（0-255）。
    pub green: u8,
    /// 蓝色分量（0-255）。
    pub blue: u8,
}

impl RGBColor {
    /// 根据分量值创建新的 RGB 颜色。
    ///
    /// # Arguments
    /// - `red` – 红色分量（0-255）。
    /// - `green` – 绿色分量（0-255）。
    /// - `blue` – 蓝色分量（0-255）。
    ///
    /// # Returns
    /// 一个新的 `RGBColor` 实例。
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// 使用欧几里得 RGB 距离查找最接近的 Minecraft 标准 16 种命名颜色。
    #[must_use]
    pub fn to_nearest_named(&self) -> NamedColor {
        const ALL_NAMED: [NamedColor; 16] = [
            NamedColor::Black,
            NamedColor::DarkBlue,
            NamedColor::DarkGreen,
            NamedColor::DarkAqua,
            NamedColor::DarkRed,
            NamedColor::DarkPurple,
            NamedColor::Gold,
            NamedColor::Gray,
            NamedColor::DarkGray,
            NamedColor::Blue,
            NamedColor::Green,
            NamedColor::Aqua,
            NamedColor::Red,
            NamedColor::LightPurple,
            NamedColor::Yellow,
            NamedColor::White,
        ];
        let mut closest = NamedColor::White;
        let mut min_dist = u32::MAX;
        for named in ALL_NAMED {
            let rgb = named.to_rgb();
            let dr = i32::from(self.red) - i32::from(rgb.red);
            let dg = i32::from(self.green) - i32::from(rgb.green);
            let db = i32::from(self.blue) - i32::from(rgb.blue);
            let dist = (dr * dr + dg * dg + db * db) as u32;
            if dist < min_dist {
                min_dist = dist;
                closest = named;
            }
        }
        closest
    }
}

impl Serialize for RGBColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!(
            "#{:02X}{:02X}{:02X}",
            self.red, self.green, self.blue
        ))
    }
}

/// 一种 ARGB 颜色，包含 Alpha、红、绿、蓝分量。
///
/// 用于自定义文本阴影等高级颜色效果。
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Deserialize)]
pub struct ARGBColor {
    /// Alpha（透明度）分量（0-255）。
    pub alpha: u8,
    /// 红色分量（0-255）。
    pub red: u8,
    /// 绿色分量（0-255）。
    pub green: u8,
    /// 蓝色分量（0-255）。
    pub blue: u8,
}

impl ARGBColor {
    /// 根据分量值创建新的 ARGB 颜色。
    ///
    /// # Arguments
    /// - `alpha` – Alpha（透明度）分量（0-255，0 为全透明，255 为不透明）。
    /// - `red` – 红色分量（0-255）。
    /// - `green` – 绿色分量（0-255）。
    /// - `blue` – 蓝色分量（0-255）。
    ///
    /// # Returns
    /// 一个新的 `ARGBColor` 实例。
    #[must_use]
    pub const fn new(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self {
            alpha,
            red,
            green,
            blue,
        }
    }

    /// 将此 ARGB 颜色转换为 32 位有符号整数（Minecraft 文本 `shadow_color` 所用的格式）。
    #[must_use]
    pub const fn to_argb_int(&self) -> i32 {
        ((self.alpha as u32) << 24
            | (self.red as u32) << 16
            | (self.green as u32) << 8
            | (self.blue as u32)) as i32
    }

    /// 将此 ARGB 颜色转换为 32 位无符号整数。
    #[must_use]
    pub const fn to_argb_u32(&self) -> u32 {
        (self.alpha as u32) << 24
            | (self.red as u32) << 16
            | (self.green as u32) << 8
            | (self.blue as u32)
    }

    /// 从 32 位无符号整数构造 `ARGBColor`。
    #[must_use]
    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            alpha: (val >> 24) as u8,
            red: (val >> 16) as u8,
            green: (val >> 8) as u8,
            blue: val as u8,
        }
    }

    /// 从 32 位有符号整数构造 `ARGBColor`。
    #[must_use]
    pub const fn from_argb_int(val: i32) -> Self {
        Self::from_argb_u32(val as u32)
    }
}

impl Serialize for ARGBColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes([self.alpha, self.red, self.green, self.blue].as_ref())
    }
}

/// 16 种标准 Minecraft 命名颜色之一。
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedColor {
    /// 黑色 (#000000)
    Black = 0,
    /// 深蓝色（#0000AA）
    DarkBlue,
    /// 深绿色（#00AA00）
    DarkGreen,
    /// 深青色（#00AAAA）
    DarkAqua,
    /// 深红色（#AA0000）
    DarkRed,
    /// 深紫色（#AA00AA）
    DarkPurple,
    /// 金色（#FFAA00）
    Gold,
    /// 灰色（#AAAAAA）
    Gray,
    /// 深灰色（#555555）
    DarkGray,
    /// 蓝色 (#5555FF)
    Blue,
    /// 绿色（#55FF55）
    Green,
    /// 青色 (#55FFFF)
    Aqua,
    /// 红色（#FF5555）
    Red,
    /// 浅紫色（#FF55FF）
    LightPurple,
    /// 黄色（#FFFF55）
    Yellow,
    /// 白色 (#FFFFFF)
    White,
}

impl NamedColor {
    /// 将此命名颜色转换为其对应的 RGB 值。
    ///
    /// # Returns
    /// 此命名颜色对应的 RGB 颜色。
    #[must_use]
    pub const fn to_rgb(&self) -> RGBColor {
        match self {
            Self::Black => RGBColor::new(0, 0, 0),
            Self::DarkBlue => RGBColor::new(0, 0, 170),
            Self::DarkGreen => RGBColor::new(0, 170, 0),
            Self::DarkAqua => RGBColor::new(0, 170, 170),
            Self::DarkRed => RGBColor::new(170, 0, 0),
            Self::DarkPurple => RGBColor::new(170, 0, 170),
            Self::Gold => RGBColor::new(255, 170, 0),
            Self::Gray => RGBColor::new(170, 170, 170),
            Self::DarkGray => RGBColor::new(85, 85, 85),
            Self::Blue => RGBColor::new(85, 85, 255),
            Self::Green => RGBColor::new(85, 255, 85),
            Self::Aqua => RGBColor::new(85, 255, 255),
            Self::Red => RGBColor::new(255, 85, 85),
            Self::LightPurple => RGBColor::new(255, 85, 255),
            Self::Yellow => RGBColor::new(255, 255, 85),
            Self::White => RGBColor::new(255, 255, 255),
        }
    }

    #[must_use]
    pub const fn to_legacy_char(&self) -> char {
        match self {
            Self::Black => '0',
            Self::DarkBlue => '1',
            Self::DarkGreen => '2',
            Self::DarkAqua => '3',
            Self::DarkRed => '4',
            Self::DarkPurple => '5',
            Self::Gold => '6',
            Self::Gray => '7',
            Self::DarkGray => '8',
            Self::Blue => '9',
            Self::Green => 'a',
            Self::Aqua => 'b',
            Self::Red => 'c',
            Self::LightPurple => 'd',
            Self::Yellow => 'e',
            Self::White => 'f',
        }
    }

    ///返回此命名颜色的 Minecraft 字符串标识符（例如 `"black"`、`"dark_blue"`）。
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::DarkBlue => "dark_blue",
            Self::DarkGreen => "dark_green",
            Self::DarkAqua => "dark_aqua",
            Self::DarkRed => "dark_red",
            Self::DarkPurple => "dark_purple",
            Self::Gold => "gold",
            Self::Gray => "gray",
            Self::DarkGray => "dark_gray",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Aqua => "aqua",
            Self::Red => "red",
            Self::LightPurple => "light_purple",
            Self::Yellow => "yellow",
            Self::White => "white",
        }
    }
}

impl std::fmt::Display for NamedColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl TryFrom<&str> for NamedColor {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "black" => Ok(Self::Black),
            "dark_blue" => Ok(Self::DarkBlue),
            "dark_green" => Ok(Self::DarkGreen),
            "dark_aqua" => Ok(Self::DarkAqua),
            "dark_red" => Ok(Self::DarkRed),
            "dark_purple" => Ok(Self::DarkPurple),
            "gold" => Ok(Self::Gold),
            "gray" => Ok(Self::Gray),
            "dark_gray" => Ok(Self::DarkGray),
            "blue" => Ok(Self::Blue),
            "green" => Ok(Self::Green),
            "aqua" => Ok(Self::Aqua),
            "red" => Ok(Self::Red),
            "light_purple" => Ok(Self::LightPurple),
            "yellow" => Ok(Self::Yellow),
            "white" => Ok(Self::White),
            _ => Err(()),
        }
    }
}
