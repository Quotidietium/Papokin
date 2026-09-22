use std::fmt::Write;

use crate::text::color::Color;
use crate::text::{TextComponent, TextContent};
use crate::translation::{Locale, get_translation_text};

impl TextComponent {
    /// 将旧版 Minecraft 格式字符串（默认使用段落符 '§'）解析为文本组件。
    ///
    /// 旧版格式化在颜色符号之后使用格式化代码：
    /// - 颜色：0-9、a-f
    /// - 样式：l（粗体）、o（斜体）、n（下划线）、m（删除线）、k（乱码）
    /// - 重置：r
    /// - RGB 十六进制颜色：§x§R§R§G§G§B§B
    ///
    /// # Arguments
    /// - `input` – 旧版格式化的字符串。
    ///
    /// # Returns
    /// 一个应用了解析所得格式的 `TextComponent`。
    #[must_use]
    pub fn from_legacy_string(input: &str) -> Self {
        Self::from_legacy_string_with_code(input, '§')
    }

    /// 使用自定义颜色代码符号（例如 '&' 或 '§'）解析旧版格式字符串。
    ///
    /// # Arguments
    /// - `input` – 旧版格式化的字符串。
    /// - `code_symbol` – 分隔符字符（例如 `'§'` 或 `'&'`）。
    ///
    /// # Returns
    /// 一个应用了解析所得格式的 `TextComponent`。
    #[must_use]
    #[expect(clippy::too_many_lines)]
    pub fn from_legacy_string_with_code(input: &str, code_symbol: char) -> Self {
        let mut root = Self::text("");
        let mut parts = input.split(code_symbol);

        if let Some(first) = parts.next()
            && !first.is_empty()
        {
            root = root.add_child(Self::text(first.to_string()));
        }

        let mut current_color: Option<Color> = None;
        let mut bold = false;
        let mut italic = false;
        let mut underlined = false;
        let mut strikethrough = false;
        let mut obfuscated = false;

        while let Some(part) = parts.next() {
            if part.is_empty() {
                continue;
            }

            let code = part.chars().next().unwrap_or(' ').to_ascii_lowercase();
            let remainder = &part[code.len_utf8()..];

            match code {
                'x' => {
                    let mut hex = [0u8; 6];
                    let mut valid_hex = true;
                    let mut trailing_remainder = String::new();
                    for hex_byte in &mut hex {
                        if let Some(next_part) = parts.next() {
                            if let Some(c) = next_part.chars().next() {
                                *hex_byte = c as u8;
                                trailing_remainder.push_str(&next_part[c.len_utf8()..]);
                            } else {
                                valid_hex = false;
                                break;
                            }
                        } else {
                            valid_hex = false;
                            break;
                        }
                    }
                    if valid_hex && let Ok(hex_str) = std::str::from_utf8(&hex) {
                        current_color = Color::from_hex_str(hex_str);
                    }
                    if !remainder.is_empty() || !trailing_remainder.is_empty() {
                        let text = format!("{remainder}{trailing_remainder}");
                        if !text.is_empty() {
                            let mut child = Self::text(text);
                            if let Some(c) = current_color {
                                child = child.color(c);
                            }
                            if bold {
                                child = child.bold();
                            }
                            if italic {
                                child = child.italic();
                            }
                            if underlined {
                                child = child.underlined();
                            }
                            if strikethrough {
                                child = child.strikethrough();
                            }
                            if obfuscated {
                                child = child.obfuscated();
                            }
                            root = root.add_child(child);
                        }
                    }
                    continue;
                }
                '0'..='9' | 'a'..='f' => {
                    current_color = Color::from_legacy_code(code);
                    bold = false;
                    italic = false;
                    underlined = false;
                    strikethrough = false;
                    obfuscated = false;
                }
                'l' => bold = true,
                'o' => italic = true,
                'n' => underlined = true,
                'm' => strikethrough = true,
                'k' => obfuscated = true,
                'r' => {
                    current_color = None;
                    bold = false;
                    italic = false;
                    underlined = false;
                    strikethrough = false;
                    obfuscated = false;
                }
                _ => {}
            }

            if !remainder.is_empty() {
                let mut child = Self::text(remainder.to_string());
                if let Some(c) = current_color {
                    child = child.color(c);
                }
                if bold {
                    child = child.bold();
                }
                if italic {
                    child = child.italic();
                }
                if underlined {
                    child = child.underlined();
                }
                if strikethrough {
                    child = child.strikethrough();
                }
                if obfuscated {
                    child = child.obfuscated();
                }
                root = root.add_child(child);
            }
        }

        root
    }

    /// 将此组件序列化为使用 `§` 的 Java 旧版格式字符串。
    ///
    /// # Arguments
    /// - `locale` – 用于解析翻译组件的区域设置。
    ///
    /// # Returns
    /// 带有 `§` 颜色和样式代码的旧版 Minecraft 格式化字符串。
    #[must_use]
    pub fn to_legacy_string(&self, locale: Locale) -> String {
        self.to_legacy_string_for_version(&crate::version::JavaMinecraftVersion::V_1_21_11, locale)
    }

    /// 将此组件序列化为特定 Minecraft 版本的 Java 旧版格式字符串。
    ///
    /// # Arguments
    /// - `version` – 要为其格式化的 Minecraft 版本。
    /// - `locale` – 用于解析翻译组件的区域设置。
    ///
    /// # Returns
    /// 带有 `§` 颜色和样式代码的旧版 Minecraft 格式化字符串。
    #[must_use]
    pub fn to_legacy_string_for_version(
        &self,
        version: &crate::version::JavaMinecraftVersion,
        locale: Locale,
    ) -> String {
        self.to_legacy_string_with_code_for_version(version, '§', locale)
    }

    /// 将此组件序列化为使用自定义颜色代码符号的旧版格式字符串。
    ///
    /// # Arguments
    /// - `code_symbol` – 格式化字符符号（例如 `'§'` 或 `'&'`）。
    /// - `locale` – 用于解析翻译组件的区域设置。
    ///
    /// # Returns
    /// 使用 `code_symbol` 作为颜色和样式代码的旧版格式化字符串。
    #[must_use]
    pub fn to_legacy_string_with_code(&self, code_symbol: char, locale: Locale) -> String {
        self.to_legacy_string_with_code_for_version(
            &crate::version::JavaMinecraftVersion::V_1_21_11,
            code_symbol,
            locale,
        )
    }

    /// 将此组件序列化为特定 Minecraft 版本的旧版格式字符串。
    ///
    /// # Arguments
    /// - `version` – 要为其格式化的 Minecraft 版本。
    /// - `code_symbol` – 格式化字符符号（例如 `'§'` 或 `'&'`）。
    /// - `locale` – 用于解析翻译组件的区域设置。
    ///
    /// # Returns
    /// 使用 `code_symbol` 作为颜色和样式代码的旧版格式化字符串。
    #[must_use]
    pub fn to_legacy_string_with_code_for_version(
        &self,
        version: &crate::version::JavaMinecraftVersion,
        code_symbol: char,
        locale: Locale,
    ) -> String {
        let mut text = String::new();

        // 1. 颜色格式化
        if let Some(color) = &self.0.style.color {
            match color {
                Color::Named(named) => {
                    let _ = write!(text, "{code_symbol}{}", named.to_legacy_char());
                }
                Color::Rgb(rgb) => {
                    if *version >= crate::version::JavaMinecraftVersion::V_1_16 {
                        let hex = format!("{:02x}{:02x}{:02x}", rgb.red, rgb.green, rgb.blue);
                        let _ = write!(text, "{code_symbol}x");
                        for ch in hex.chars() {
                            let _ = write!(text, "{code_symbol}{ch}");
                        }
                    } else {
                        let named = rgb.to_nearest_named();
                        let _ = write!(text, "{code_symbol}{}", named.to_legacy_char());
                    }
                }
                Color::Reset => {
                    let _ = write!(text, "{code_symbol}r");
                }
            }
        }

        // 2. 样式格式化
        if self.0.style.bold == Some(true) {
            let _ = write!(text, "{code_symbol}l");
        }
        if self.0.style.italic == Some(true) {
            let _ = write!(text, "{code_symbol}o");
        }
        if self.0.style.underlined == Some(true) {
            let _ = write!(text, "{code_symbol}n");
        }
        if self.0.style.strikethrough == Some(true) {
            let _ = write!(text, "{code_symbol}m");
        }
        if self.0.style.obfuscated == Some(true) {
            let _ = write!(text, "{code_symbol}k");
        }

        // 3. 解析内容
        match &*self.0.content {
            TextContent::Text { text: t } => text.push_str(t),
            TextContent::Translate { translate, with } => {
                text.push_str(&get_translation_text(
                    format!("minecraft:{translate}"),
                    locale,
                    with.clone(),
                ));
            }
            TextContent::EntityNames { selector, .. } => text.push_str(selector),
            TextContent::Keybind { keybind } => text.push_str(keybind),
            TextContent::Custom { key, with, .. } => {
                text.push_str(&get_translation_text(key.clone(), locale, with.clone()));
            }
            TextContent::PlayerSprite { profile, .. } => {
                if let Some(name) = profile.0.get_string("name") {
                    text.push_str(name);
                }
            }
        }

        // 4. 递归追加额外组件
        for child in &self.0.extra {
            text.push_str(&Self(child.clone()).to_legacy_string_with_code_for_version(
                version,
                code_symbol,
                locale,
            ));
            let _ = write!(text, "{code_symbol}r");
        }

        text
    }
}

#[cfg(test)]
mod test {
    use crate::text::TextComponent;
    use crate::text::color::{Color, NamedColor};
    use crate::translation::Locale;

    #[test]
    fn from_legacy_string_hex() {
        let comp = TextComponent::from_legacy_string("§x§f§c§0§0§0§0Test String ▽");
        assert_eq!(comp.0.extra.len(), 1);
        if let crate::text::TextContent::Text { text } = &*comp.0.extra[0].content {
            assert_eq!(text, "Test String ▽");
        } else {
            panic!("Expected Text content");
        }
        assert_eq!(comp.0.extra[0].style.color, Color::from_hex_str("fc0000"));
    }

    #[test]
    fn from_legacy_string_alt_code() {
        let comp = TextComponent::from_legacy_string_with_code("&cRed &lBold", '&');
        assert_eq!(comp.0.extra.len(), 2);
        assert_eq!(
            comp.0.extra[0].style.color,
            Some(Color::Named(NamedColor::Red))
        );
        assert_eq!(comp.0.extra[1].style.bold, Some(true));
    }

    #[test]
    fn to_legacy_string() {
        let comp = TextComponent::text("你好 ").add_child(
            TextComponent::text("世界")
                .color_named(NamedColor::Red)
                .bold(),
        );
        let legacy = comp.to_legacy_string(Locale::EnUs);
        assert_eq!(legacy, "你好 §c§l世界§r");
    }
}
