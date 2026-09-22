use crate::entity::player::{Player, TitleMode};
use papokin_util::text::TextComponent;

/// 一个流式构建器，用于构建并显示标题、副标题和动作栏文本，并支持设置动画时间。
#[derive(Debug, Clone, Default)]
pub struct TitleBuilder {
    title: Option<TextComponent>,
    subtitle: Option<TextComponent>,
    actionbar: Option<TextComponent>,
    fade_in: Option<i32>,
    stay: Option<i32>,
    fade_out: Option<i32>,
}

impl TitleBuilder {
    /// 创建新的 `TitleBuilder`。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置主标题文本组件。
    #[must_use]
    pub fn title(mut self, title: impl Into<TextComponent>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 从纯文本设置主标题。
    #[must_use]
    pub fn title_text(mut self, text: impl Into<String>) -> Self {
        self.title = Some(TextComponent::text(text.into()));
        self
    }

    /// 设置副标题文本组件。
    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<TextComponent>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// 从纯文本设置副标题。
    #[must_use]
    pub fn subtitle_text(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(TextComponent::text(text.into()));
        self
    }

    /// 设置动作栏文本组件。
    #[must_use]
    pub fn actionbar(mut self, actionbar: impl Into<TextComponent>) -> Self {
        self.actionbar = Some(actionbar.into());
        self
    }

    /// 从纯文本设置动作栏文本。
    #[must_use]
    pub fn actionbar_text(mut self, text: impl Into<String>) -> Self {
        self.actionbar = Some(TextComponent::text(text.into()));
        self
    }

    /// 以刻为单位设置淡入、停留与淡出的时长。
    #[must_use]
    pub const fn times(mut self, fade_in: i32, stay: i32, fade_out: i32) -> Self {
        self.fade_in = Some(fade_in);
        self.stay = Some(stay);
        self.fade_out = Some(fade_out);
        self
    }

    /// 将标题配置发送给指定玩家。
    pub fn send_to(&self, player: &Player) {
        if let (Some(fade_in), Some(stay), Some(fade_out)) =
            (self.fade_in, self.stay, self.fade_out)
        {
            player.send_title_animation(fade_in, stay, fade_out);
        }

        if let Some(ref title) = self.title {
            player.show_title(title, &TitleMode::Title);
        }

        if let Some(ref subtitle) = self.subtitle {
            player.show_title(subtitle, &TitleMode::SubTitle);
        }

        if let Some(ref actionbar) = self.actionbar {
            player.show_title(actionbar, &TitleMode::ActionBar);
        }
    }
}
