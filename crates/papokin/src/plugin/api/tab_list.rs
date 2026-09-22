use papokin_util::text::TextComponent;

/// 表示 Java 版的 Tab 列表配置，包含页眉和页脚组件。
#[derive(Debug, Clone)]
pub struct TabList {
    /// Tab 列表的页眉组件。
    pub header: TextComponent,
    /// Tab 列表的页脚组件。
    pub footer: TextComponent,
}

impl Default for TabList {
    fn default() -> Self {
        Self {
            header: TextComponent::text(""),
            footer: TextComponent::text(""),
        }
    }
}

impl TabList {
    /// 创建新的 `TabListBuilder`。
    #[must_use]
    pub fn builder() -> TabListBuilder {
        TabListBuilder::default()
    }
}

/// `TabList` 的流式构建器。
#[derive(Debug, Clone, Default)]
pub struct TabListBuilder {
    header: Option<TextComponent>,
    footer: Option<TextComponent>,
}

impl TabListBuilder {
    /// 创建新的 `TabListBuilder`。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 Tab 列表页眉组件。
    #[must_use]
    pub fn header(mut self, header: impl Into<TextComponent>) -> Self {
        self.header = Some(header.into());
        self
    }

    /// 从纯文本设置 Tab 列表页眉。
    #[must_use]
    pub fn header_text(mut self, text: impl Into<String>) -> Self {
        self.header = Some(TextComponent::text(text.into()));
        self
    }

    /// 设置 Tab 列表页脚组件。
    #[must_use]
    pub fn footer(mut self, footer: impl Into<TextComponent>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// 从纯文本设置 Tab 列表页脚。
    #[must_use]
    pub fn footer_text(mut self, text: impl Into<String>) -> Self {
        self.footer = Some(TextComponent::text(text.into()));
        self
    }

    /// 构建 `TabList`。
    #[must_use]
    pub fn build(self) -> TabList {
        TabList {
            header: self.header.unwrap_or_else(|| TextComponent::text("")),
            footer: self.footer.unwrap_or_else(|| TextComponent::text("")),
        }
    }
}

impl From<TabListBuilder> for TabList {
    fn from(builder: TabListBuilder) -> Self {
        builder.build()
    }
}
