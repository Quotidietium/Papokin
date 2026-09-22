use super::{
    click::ClickEvent,
    color::{self, Color},
    hover::HoverEvent,
};
use crate::text::color::ARGBColor;
use serde::{Deserialize, Serialize};

/// 表示文本组件的样式选项。
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Style {
    /// 渲染内容所使用的颜色。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// 是否以粗体渲染内容。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// 是否以斜体渲染内容。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// 是否以下划线渲染内容。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlined: Option<bool>,
    /// 是否以删除线渲染内容。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    /// 是否以混淆样式渲染内容。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfuscated: Option<bool>,
    /// 当玩家按住 Shift 点击文本时，该字符串会被插入玩家的聊天输入中。它不会覆盖玩家正在输入的任何现有文本。仅对聊天消息有效。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<String>,
    /// 允许在玩家点击文本时触发事件。仅在聊天中有效。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click_event: Option<ClickEvent>,
    /// 允许在玩家将鼠标悬停在文本上时显示提示框。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover_event: Option<HoverEvent>,
    /// 允许你更改文本的字体。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// 文本的自定义阴影颜色。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "shadow_color"
    )]
    pub shadow_color: Option<ARGBColor>,
}

impl Style {
    ///若未设置任何样式选项，则返回 `true`。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.underlined.is_none()
            && self.strikethrough.is_none()
            && self.obfuscated.is_none()
            && self.insertion.is_none()
            && self.click_event.is_none()
            && self.hover_event.is_none()
            && self.font.is_none()
            && self.shadow_color.is_none()
    }

    /// 使用 `Color` 枚举值设置文本颜色。
    ///
    /// # Arguments
    /// - `color` – 要应用的颜色。
    ///
    /// # Returns
    /// 设置了颜色的样式实例。
    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// 使用 Minecraft 命名颜色设置文本颜色。
    ///
    /// # Arguments
    /// - `color` – 要应用的命名颜色（例如 `NamedColor::Red`）。
    ///
    /// # Returns
    /// 设置了命名颜色的样式实例。
    #[must_use]
    pub const fn color_named(mut self, color: color::NamedColor) -> Self {
        self.color = Some(Color::Named(color));
        self
    }

    /// 使文本变为粗体。
    ///
    /// # Returns
    /// 启用了粗体的样式实例。
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }

    /// 使文本变为斜体。
    ///
    /// # Returns
    /// 启用了斜体的样式实例。
    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.italic = Some(true);
        self
    }

    /// 为文本添加下划线。
    ///
    /// # Returns
    /// 启用了下划线的样式实例。
    #[must_use]
    pub const fn underlined(mut self) -> Self {
        self.underlined = Some(true);
        self
    }

    /// 为文本添加删除线。
    ///
    /// # Returns
    /// 启用了删除线的样式实例。
    #[must_use]
    pub const fn strikethrough(mut self) -> Self {
        self.strikethrough = Some(true);
        self
    }

    /// 使文本变为乱码（随机字符）。
    ///
    /// # Returns
    /// 启用了混淆的样式实例。
    #[must_use]
    pub const fn obfuscated(mut self) -> Self {
        self.obfuscated = Some(true);
        self
    }

    /// 设置按住 Shift 点击时插入玩家聊天输入框的文本。
    ///
    /// # Arguments
    /// - `text` – 按住 Shift 点击时要插入的文本。
    ///
    /// # Returns
    /// 设置了插入文本的样式实例。
    #[must_use]
    pub fn insertion(mut self, text: String) -> Self {
        self.insertion = Some(text);
        self
    }

    /// 设置玩家点击文本时发生的事件。
    ///
    /// # Arguments
    /// - `event` – 要触发的点击事件。
    ///
    /// # Returns
    /// 设置了点击事件的样式实例。
    #[must_use]
    pub fn click_event(mut self, event: ClickEvent) -> Self {
        self.click_event = Some(event);
        self
    }

    /// 设置玩家悬停在文本上时显示的工具提示。
    ///
    /// # Arguments
    /// - `event` – 要显示的悬停事件。
    ///
    /// # Returns
    /// 设置了悬停事件的样式实例。
    #[must_use]
    pub fn hover_event(mut self, event: HoverEvent) -> Self {
        self.hover_event = Some(event);
        self
    }

    /// 设置用于渲染的字体资源位置。
    ///
    /// 允许更改文本的字体。默认字体包括：
    /// - `minecraft:default` - Minecraft 标准字体
    /// - `minecraft:uniform` - 等宽字体
    /// - `minecraft:alt` - 一种备选字体样式
    /// - `minecraft:illageralt` - 灾厄村民主题字体
    ///
    /// # Arguments
    /// - `resource_location` – 字体资源位置（例如 "minecraft:uniform"）。
    ///
    /// # Returns
    /// 设置了字体的样式实例。
    #[must_use]
    pub fn font(mut self, resource_location: String) -> Self {
        self.font = Some(resource_location);
        self
    }

    /// 覆盖文本的阴影颜色。
    ///
    /// # Arguments
    /// - `color` – 阴影的 ARGB 颜色值。
    ///
    /// # Returns
    /// 设置了阴影颜色的样式实例。
    #[must_use]
    pub const fn shadow_color(mut self, color: ARGBColor) -> Self {
        self.shadow_color = Some(color);
        self
    }
}
