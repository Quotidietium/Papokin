use std::{
    borrow::Cow,
    collections::HashMap,
    str::FromStr,
    sync::{LazyLock, Mutex},
};

/// TODO 列表
/// - 添加服务器语言环境支持
/// - 在日志中使用翻译
/// - 开放公开的翻译系统，或许像 Minecraft 一样使用 Crowdin？
/// - 支持对命令描述进行翻译
/// - 将自定义翻译与插件 API 集成
/// - 尝试优化 '`to_translated`' 的代码
use crate::text::{TextComponentBase, TextContent, style::Style};

static VANILLA_EN_US_JSON: &str = include_str!("../../../assets/en_us_java.json");
static PAPOKIN_EN_US_JSON: &str = include_str!("../../../assets/translations/en_us.json");
static PAPOKIN_BRB_JSON: &str = include_str!("../../../assets/translations/brb.json");
static PAPOKIN_DE_DE_JSON: &str = include_str!("../../../assets/translations/de_de.json");
static PAPOKIN_ES_ES_JSON: &str = include_str!("../../../assets/translations/es_es.json");
static PAPOKIN_FR_FR_JSON: &str = include_str!("../../../assets/translations/fr_fr.json");
static PAPOKIN_HR_HR_JSON: &str = include_str!("../../../assets/translations/hr_hr.json");
static PAPOKIN_IT_IT_JSON: &str = include_str!("../../../assets/translations/it_it.json");
static PAPOKIN_JA_JP_JSON: &str = include_str!("../../../assets/translations/ja_jp.json");
static PAPOKIN_KA_GE_JSON: &str = include_str!("../../../assets/translations/ka_ge.json");
static PAPOKIN_KO_KR_JSON: &str = include_str!("../../../assets/translations/ko_kr.json");
static PAPOKIN_NDS_DE_JSON: &str = include_str!("../../../assets/translations/nds_de.json");
static PAPOKIN_NL_BE_JSON: &str = include_str!("../../../assets/translations/nl_be.json");
static PAPOKIN_NL_NL_JSON: &str = include_str!("../../../assets/translations/nl_nl.json");
static PAPOKIN_RO_RO_JSON: &str = include_str!("../../../assets/translations/ro_ro.json");
static PAPOKIN_RU_RU_JSON: &str = include_str!("../../../assets/translations/ru_ru.json");
static PAPOKIN_SQ_AL_JSON: &str = include_str!("../../../assets/translations/sq_al.json");
static PAPOKIN_ZH_CN_JSON: &str = include_str!("../../../assets/translations/zh_cn.json");
static PAPOKIN_ZH_HK_JSON: &str = include_str!("../../../assets/translations/zh_hk.json");
static PAPOKIN_ZH_TW_JSON: &str = include_str!("../../../assets/translations/zh_tw.json");
static PAPOKIN_LZH_JSON: &str = include_str!("../../../assets/translations/lzh.json");
static PAPOKIN_TR_TR_JSON: &str = include_str!("../../../assets/translations/tr_tr.json");
static PAPOKIN_UK_UA_JSON: &str = include_str!("../../../assets/translations/uk_ua.json");
static PAPOKIN_VI_VN_JSON: &str = include_str!("../../../assets/translations/vi_vn.json");
static PAPOKIN_PT_BR_JSON: &str = include_str!("../../../assets/translations/pt_br.json");
static PAPOKIN_PL_PL_JSON: &str = include_str!("../../../assets/translations/pl_pl.json");

/// 一个字符范围，表示翻译字符串内的替换占位符。
///
/// 该范围包含两端，对应占位符的完整跨度
/// (例如 `%s` 或 `%1$s`)。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SubstitutionRange {
    /// 起始字节索引（含）。
    pub start: usize,
    /// 结束字节索引（含）。
    pub end: usize,
}
impl SubstitutionRange {
    /// 返回该范围的长度。
    #[must_use]
    pub const fn len(&self) -> usize {
        (self.end - self.start) + 1
    }
    ///若该范围不包含任何字符，则返回 `true`。
    ///
    /// 当 `start == end` 时，该范围被视为空。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// 添加或覆盖单个翻译条目。
///
/// # Arguments
/// * `namespace`: 翻译键的命名空间。
/// * `key`: 不含命名空间的翻译键。
/// * `translation`: 本地化后的翻译字符串。
/// * `locale`: 该翻译所属的语言环境。
pub fn add_translation<P: Into<String>>(namespace: P, key: P, translation: P, locale: Locale) {
    let mut translations = TRANSLATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let namespaced_key = format!("{}:{}", namespace.into(), key.into()).to_lowercase();
    translations[locale as usize].insert(namespaced_key, translation.into());
}

/// 从 JSON 字符串加载翻译并在某个命名空间下注册。
///
/// # Arguments
/// * `namespace`: 应用于所有已加载键的命名空间。
/// * `file_path`: 一个 JSON 字符串，包含扁平的键值对翻译映射。
/// * `locale`: 这些翻译所属的语言环境。
pub fn add_translation_file<P: Into<String>>(namespace: P, file_path: P, locale: Locale) {
    let translations_map: HashMap<String, String> =
        serde_json::from_str(&file_path.into()).unwrap_or_default();
    if translations_map.is_empty() {
        // TODO: 妥善处理文件为空或未找到的情况
        return;
    }

    let mut translations = TRANSLATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let namespace = namespace.into();
    for (key, translation) in translations_map {
        let namespaced_key = format!("{namespace}:{key}").to_lowercase();
        translations[locale as usize].insert(namespaced_key, translation);
    }
}

/// 检索给定键和区域设置对应的翻译。
///
/// # Arguments
/// * `key`: 完整限定的 `namespace:key`。
/// * `locale`: 请求的语言环境。
///
/// # Returns
/// 本地化后的翻译。未找到时回退到 `en_us` 或键本身。
pub fn get_translation(key: &str, locale: Locale) -> String {
    let translations = TRANSLATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let key = key.to_lowercase();
    translations[locale as usize].get(&key).map_or_else(
        || {
            translations[Locale::EnUs as usize]
                .get(&key)
                .map_or(key, Clone::clone)
        },
        Clone::clone,
    )
}

/// 翻译字符串中 `%` 引出的内容。
enum Placeholder {
    /// `%%`，表示单个字面量 `%`。
    Escape,
    /// `%<conversion>`，按顺序取下一个替换项。
    Next,
    /// `%<digits>$<conversion>`，取该从 1 开始计数的
    /// 索引。
    Indexed(usize),
}

/// `byte` 是否用于关闭占位符。随附的表格使用 `s`，且每个
/// 转换同样会在此处替换，因此该字母仅用于标记
/// 令牌的末尾。
const fn is_conversion(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

/// 对 `start` 处的 `%` 进行分类，返回其所引入的内容及
/// 它结束处的字节索引（含）。
///
///当 `%` 之后不是本模块所能理解的内容时，返回 `None`
/// 尾随的 `%` 或其后没有转换符的数字；那些是字面文本。
/// 之所以报告结束位置而不是直接假定，是因为 [`SubstitutionRange`] 必须
/// 覆盖整个占位符，供从 `end + 1` 恢复读取的调用方使用。
fn placeholder_at(bytes: &[u8], start: usize) -> Option<(Placeholder, usize)> {
    let after = start + 1;
    match bytes.get(after) {
        Some(&b'%') => return Some((Placeholder::Escape, after)),
        Some(&byte) if is_conversion(byte) => return Some((Placeholder::Next, after)),
        _ => {}
    }

    let mut end = after;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == after
        || bytes.get(end) != Some(&b'$')
        || !bytes.get(end + 1).copied().is_some_and(is_conversion)
    {
        return None;
    }

    // 饱和处理，使极长的连续数字仍落在最后一个
    // 替代处理，而不是让解析失败。
    let index = bytes[after..end].iter().fold(0usize, |index, digit| {
        index
            .saturating_mul(10)
            .saturating_add(usize::from(digit - b'0'))
    });
    Some((Placeholder::Indexed(index), end + 1))
}

/// 重新排列翻译字符串中的替换占位符。
///
/// # Arguments
/// * `translation`: 包含占位符的原始翻译字符串。
/// * `with`: 要插入占位符的替换组件。
///
/// # Returns
/// 一个包含重排后组件及其替换范围的元组。
#[must_use]
pub fn reorder_substitutions(
    translation: &str,
    with: Vec<TextComponentBase>,
) -> (Vec<TextComponentBase>, Vec<SubstitutionRange>) {
    fn literal(text: &'static str) -> TextComponentBase {
        TextComponentBase {
            content: Box::new(TextContent::Text { text: text.into() }),
            style: Box::new(Style::default()),
            extra: vec![],
        }
    }

    // 占位符可能重复某个索引（`%1$s ... %1$s`），因此参数
    // 被读取而非被耗尽。为此只读访问将其冻结。
    let with = with.into_boxed_slice();

    let bytes = translation.as_bytes();
    let mut substitutions: Vec<TextComponentBase> = vec![];
    let mut ranges: Vec<SubstitutionRange> = vec![];
    let mut next_idx = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'%' || (i > 0 && bytes[i - 1] == b'\\') {
            i += 1;
            continue;
        }
        let Some((placeholder, end)) = placeholder_at(bytes, i) else {
            i += 1;
            continue;
        };

        substitutions.push(match placeholder {
            Placeholder::Escape => literal("%"),
            Placeholder::Next => {
                let taken = with.get(next_idx).cloned().unwrap_or_else(|| literal(""));
                next_idx = (next_idx + 1).min(with.len().saturating_sub(1));
                taken
            }
            Placeholder::Indexed(index) => with
                .get(index.clamp(1, with.len().max(1)) - 1)
                .cloned()
                .unwrap_or_else(|| literal("")),
        });
        ranges.push(SubstitutionRange { start: i, end });
        i = end + 1;
    }

    (substitutions, ranges)
}

/// 将翻译解析为带格式的控制台输出。
///
/// # Arguments
/// * `namespaced_key`: 完整限定的 `namespace:key`。
/// * `locale`: 请求的语言环境。
/// * `with`: 用于替换占位符的替换组件。
///
/// # Returns
/// 已解析并格式化的翻译字符串。
pub fn translation_to_pretty<P: Into<Cow<'static, str>>>(
    namespaced_key: P,
    locale: Locale,
    with: Vec<TextComponentBase>,
) -> String {
    let translation = get_translation(&namespaced_key.into(), locale);
    if with.is_empty() || !translation.contains('%') {
        return translation;
    }

    let (substitutions, indices) = reorder_substitutions(&translation, with);
    let mut result = String::new();
    let mut pos = 0;

    for (idx, &range) in indices.iter().enumerate() {
        let sub_idx = idx.clamp(0, substitutions.len() - 1);
        let substitution = substitutions[sub_idx].clone().to_pretty_console();

        result.push_str(&translation[pos..range.start]);
        result.push_str(&substitution);
        pos = range.end + 1;
    }

    result.push_str(&translation[pos..]);
    result
}

/// 将翻译解析为纯文本。
///
/// # Arguments
/// * `namespaced_key`: 完整限定的 `namespace:key`。
/// * `locale`: 请求的语言环境。
/// * `with`: 用于替换占位符的替换组件。
///
/// # Returns
/// 解析后的翻译（纯文本形式）。
pub fn get_translation_text<P: Into<Cow<'static, str>>>(
    namespaced_key: P,
    locale: Locale,
    with: Vec<TextComponentBase>,
) -> String {
    let translation = get_translation(&namespaced_key.into(), locale);
    if with.is_empty() || !translation.contains('%') {
        return translation;
    }

    let (substitutions, indices) = reorder_substitutions(&translation, with);
    let mut result = String::new();
    let mut pos = 0;

    for (idx, &range) in indices.iter().enumerate() {
        let sub_idx = idx.clamp(0, substitutions.len() - 1);
        let substitution = substitutions[sub_idx].clone().get_text(locale);

        result.push_str(&translation[pos..range.start]);
        result.push_str(&substitution);
        pos = range.end + 1;
    }

    result.push_str(&translation[pos..]);
    result
}

pub static TRANSLATIONS: LazyLock<Mutex<[HashMap<String, String>; Locale::COUNT]>> =
    LazyLock::new(|| {
        let mut array: [HashMap<String, String>; Locale::COUNT] =
            std::array::from_fn(|_| HashMap::new());
        let parse_json = |json: &str| -> HashMap<String, String> {
            serde_json::from_str(json).unwrap_or_default()
        };
        let vanilla_en_us = parse_json(VANILLA_EN_US_JSON);
        let papokin_en_us = parse_json(PAPOKIN_EN_US_JSON);
        let papokin_brb = parse_json(PAPOKIN_BRB_JSON);
        let papokin_de_de = parse_json(PAPOKIN_DE_DE_JSON);
        let papokin_es_es = parse_json(PAPOKIN_ES_ES_JSON);
        let papokin_fr_fr = parse_json(PAPOKIN_FR_FR_JSON);
        let papokin_hr_hr = parse_json(PAPOKIN_HR_HR_JSON);
        let papokin_it_it = parse_json(PAPOKIN_IT_IT_JSON);
        let papokin_ja_jp = parse_json(PAPOKIN_JA_JP_JSON);
        let papokin_ka_ge = parse_json(PAPOKIN_KA_GE_JSON);
        let papokin_ko_kr = parse_json(PAPOKIN_KO_KR_JSON);
        let papokin_nds_de = parse_json(PAPOKIN_NDS_DE_JSON);
        let papokin_nl_be = parse_json(PAPOKIN_NL_BE_JSON);
        let papokin_nl_nl = parse_json(PAPOKIN_NL_NL_JSON);
        let papokin_ro_ro = parse_json(PAPOKIN_RO_RO_JSON);
        let papokin_ru_ru = parse_json(PAPOKIN_RU_RU_JSON);
        let papokin_sq_al = parse_json(PAPOKIN_SQ_AL_JSON);
        let papokin_zh_cn = parse_json(PAPOKIN_ZH_CN_JSON);
        let papokin_zh_hk = parse_json(PAPOKIN_ZH_HK_JSON);
        let papokin_zh_tw = parse_json(PAPOKIN_ZH_TW_JSON);
        let papokin_lzh = parse_json(PAPOKIN_LZH_JSON);
        let papokin_tr_tr = parse_json(PAPOKIN_TR_TR_JSON);
        let papokin_uk_ua = parse_json(PAPOKIN_UK_UA_JSON);
        let papokin_vi_vn = parse_json(PAPOKIN_VI_VN_JSON);
        let papokin_pt_br = parse_json(PAPOKIN_PT_BR_JSON);
        let papokin_pl_pl = parse_json(PAPOKIN_PL_PL_JSON);

        for (key, value) in vanilla_en_us {
            array[Locale::EnUs as usize].insert(format!("minecraft:{key}"), value);
        }
        for (key, value) in papokin_en_us {
            array[Locale::EnUs as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_brb {
            array[Locale::Brb as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_de_de {
            array[Locale::DeDe as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_es_es {
            array[Locale::EsEs as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_fr_fr {
            array[Locale::FrFr as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_hr_hr {
            array[Locale::HrHr as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_it_it {
            array[Locale::ItIt as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_ja_jp {
            array[Locale::JaJp as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_ka_ge {
            array[Locale::KaGe as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_ko_kr {
            array[Locale::KoKr as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_nds_de {
            array[Locale::NdsDe as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_nl_be {
            array[Locale::NlBe as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_nl_nl {
            array[Locale::NlNl as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_ro_ro {
            array[Locale::RoRo as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_ru_ru {
            array[Locale::RuRu as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_sq_al {
            array[Locale::SqAl as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_zh_cn {
            array[Locale::ZhCn as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_zh_hk {
            array[Locale::ZhHk as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_zh_tw {
            array[Locale::ZhTw as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_lzh {
            array[Locale::Lzh as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_tr_tr {
            array[Locale::TrTr as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_uk_ua {
            array[Locale::UkUa as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_vi_vn {
            array[Locale::ViVn as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_pt_br {
            array[Locale::PtBr as usize].insert(format!("papokin:{key}"), value);
        }
        for (key, value) in papokin_pl_pl {
            array[Locale::PlPl as usize].insert(format!("papokin:{key}"), value);
        }

        Mutex::new(array)
    });

/// 翻译所支持的区域设置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    AfZa,
    ArSa,
    AstEs,
    AzAz,
    BaRu,
    Bar,
    BeBy,
    BgBg,
    BrFr,
    Brb,
    BsBa,
    CaEs,
    CsCz,
    CyGb,
    DaDk,
    DeAt,
    DeCh,
    DeDe,
    ElGr,
    EnAu,
    EnCa,
    EnGb,
    EnNz,
    EnPt,
    EnUd,
    EnUs,
    Enp,
    Enws,
    EoUy,
    EsAr,
    EsCl,
    EsEc,
    EsEs,
    EsMx,
    EsUy,
    EsVe,
    Esan,
    EtEe,
    EuEs,
    FaIr,
    FiFi,
    FilPh,
    FoFo,
    FrCa,
    FrFr,
    FraDe,
    FurIt,
    FyNl,
    GaIe,
    GdGb,
    GlEs,
    HawUs,
    HeIl,
    HiIn,
    HrHr,
    HuHu,
    HyAm,
    IdId,
    IgNg,
    IoEn,
    IsIs,
    Isv,
    ItIt,
    JaJp,
    JboEn,
    KaGe,
    KkKz,
    KnIn,
    KoKr,
    Ksh,
    KwGb,
    LaLa,
    LbLu,
    LiLi,
    Lmo,
    LoLa,
    LolUs,
    LtLt,
    LvLv,
    Lzh,
    MkMk,
    MnMn,
    MsMy,
    MtMt,
    Nah,
    NdsDe,
    NlBe,
    NlNl,
    NnNo,
    NoNo,
    OcFr,
    Ovd,
    PlPl,
    PtBr,
    PtPt,
    QyaAa,
    RoRo,
    Rpr,
    RuRu,
    RyUa,
    SahSah,
    SeNo,
    SkSk,
    SlSi,
    SoSo,
    SqAl,
    SrCs,
    SrSp,
    SvSe,
    Sxu,
    Szl,
    TaIn,
    ThTh,
    TlPh,
    TlhAa,
    Tok,
    TrTr,
    TtRu,
    UkUa,
    ValEs,
    VecIt,
    ViVn,
    YiDe,
    YoNg,
    ZhCn,
    ZhHk,
    ZhTw,
    ZlmArab,
}

impl Locale {
    pub const COUNT: usize = Self::ZlmArab as usize + 1;
}

impl FromStr for Locale {
    type Err = ();

    #[expect(clippy::too_many_lines)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "af_za" => Ok(Self::AfZa),       // 南非荷兰语（南非）
            "ar_sa" => Ok(Self::ArSa),       // 阿拉伯语
            "ast_es" => Ok(Self::AstEs),     // 阿斯图里亚斯语
            "az_az" => Ok(Self::AzAz),       // 阿塞拜疆语
            "ba_ru" => Ok(Self::BaRu),       // 巴什基尔语
            "bar" => Ok(Self::Bar),          // 巴伐利亚语
            "be_by" => Ok(Self::BeBy),       // 白俄罗斯语
            "bg_bg" => Ok(Self::BgBg),       // 保加利亚语
            "br_fr" => Ok(Self::BrFr),       // 布列塔尼语
            "brb" => Ok(Self::Brb),          // 布拉班特语
            "bs_ba" => Ok(Self::BsBa),       // 波斯尼亚语
            "ca_es" => Ok(Self::CaEs),       // 加泰罗尼亚语
            "cs_cz" => Ok(Self::CsCz),       // 捷克语
            "cy_gb" => Ok(Self::CyGb),       // 威尔士语
            "da_dk" => Ok(Self::DaDk),       // 丹麦语
            "de_at" => Ok(Self::DeAt),       // 奥地利德语
            "de_ch" => Ok(Self::DeCh),       // 瑞士德语
            "de_de" => Ok(Self::DeDe),       // 德语
            "el_gr" => Ok(Self::ElGr),       // 希腊语
            "en_au" => Ok(Self::EnAu),       // 澳大利亚英语
            "en_ca" => Ok(Self::EnCa),       // 加拿大英语
            "en_gb" => Ok(Self::EnGb),       // 英式英语
            "en_nz" => Ok(Self::EnNz),       // 新西兰英语
            "en_pt" => Ok(Self::EnPt),       // 海盗英语
            "en_ud" => Ok(Self::EnUd),       // 上下颠倒的英式英语
            "enp" => Ok(Self::Enp),          // 现代英语减去外来借词
            "enws" => Ok(Self::Enws),        // 早期现代英语
            "eo_uy" => Ok(Self::EoUy),       // 世界语
            "es_ar" => Ok(Self::EsAr),       // 阿根廷西班牙语
            "es_cl" => Ok(Self::EsCl),       // 智利西班牙语
            "es_ec" => Ok(Self::EsEc),       // 厄瓜多尔西班牙语
            "es_es" => Ok(Self::EsEs),       // 欧洲西班牙语
            "es_mx" => Ok(Self::EsMx),       // 墨西哥西班牙语
            "es_uy" => Ok(Self::EsUy),       // 乌拉圭西班牙语
            "es_ve" => Ok(Self::EsVe),       // 委内瑞拉西班牙语
            "esan" => Ok(Self::Esan),        // 安达卢西亚语
            "et_ee" => Ok(Self::EtEe),       // 爱沙尼亚语
            "eu_es" => Ok(Self::EuEs),       // 巴斯克语
            "fa_ir" => Ok(Self::FaIr),       // 波斯语
            "fi_fi" => Ok(Self::FiFi),       // 芬兰语
            "fil_ph" => Ok(Self::FilPh),     // 菲律宾语
            "fo_fo" => Ok(Self::FoFo),       // 法罗语
            "fr_ca" => Ok(Self::FrCa),       // 加拿大法语
            "fr_fr" => Ok(Self::FrFr),       // 欧洲法语
            "fra_de" => Ok(Self::FraDe),     // 东法兰克语
            "fur_it" => Ok(Self::FurIt),     // 弗留利语
            "fy_nl" => Ok(Self::FyNl),       // 弗里斯兰语
            "ga_ie" => Ok(Self::GaIe),       // 爱尔兰语
            "gd_gb" => Ok(Self::GdGb),       // 苏格兰盖尔语
            "gl_es" => Ok(Self::GlEs),       // 加利西亚语
            "haw_us" => Ok(Self::HawUs),     // 夏威夷语
            "he_il" => Ok(Self::HeIl),       // 希伯来语
            "hi_in" => Ok(Self::HiIn),       // 印地语
            "hr_hr" => Ok(Self::HrHr),       // 克罗地亚语
            "hu_hu" => Ok(Self::HuHu),       // 匈牙利语
            "hy_am" => Ok(Self::HyAm),       // 亚美尼亚语
            "id_id" => Ok(Self::IdId),       // 印尼语
            "ig_ng" => Ok(Self::IgNg),       // 伊博语
            "io_en" => Ok(Self::IoEn),       // 伊多语
            "is_is" => Ok(Self::IsIs),       // 冰岛语
            "isv" => Ok(Self::Isv),          // 国际斯拉夫语
            "it_it" => Ok(Self::ItIt),       // 意大利语
            "ja_jp" => Ok(Self::JaJp),       // 日语
            "jbo_en" => Ok(Self::JboEn),     // Lojban
            "ka_ge" => Ok(Self::KaGe),       // 格鲁吉亚语
            "kk_kz" => Ok(Self::KkKz),       // 哈萨克语
            "kn_in" => Ok(Self::KnIn),       // 卡纳达语
            "ko_kr" => Ok(Self::KoKr),       // 韩语
            "ksh" => Ok(Self::Ksh),          // 科隆语/里普阿里安语
            "kw_gb" => Ok(Self::KwGb),       // 康沃尔语
            "la_la" => Ok(Self::LaLa),       // 拉丁语
            "lb_lu" => Ok(Self::LbLu),       // 卢森堡语
            "li_li" => Ok(Self::LiLi),       // 林堡语
            "lmo" => Ok(Self::Lmo),          // 伦巴第语
            "lo_la" => Ok(Self::LoLa),       // 老挝语
            "lol_us" => Ok(Self::LolUs),     // LOLCAT
            "lt_lt" => Ok(Self::LtLt),       // 立陶宛语
            "lv_lv" => Ok(Self::LvLv),       // 拉脱维亚语
            "lzh" => Ok(Self::Lzh),          // 文言文
            "mk_mk" => Ok(Self::MkMk),       // 马其顿语
            "mn_mn" => Ok(Self::MnMn),       // 蒙古语
            "ms_my" => Ok(Self::MsMy),       // 马来语
            "mt_mt" => Ok(Self::MtMt),       // 马耳他语
            "nah" => Ok(Self::Nah),          // 纳瓦特尔语
            "nds_de" => Ok(Self::NdsDe),     // 低地德语
            "nl_be" => Ok(Self::NlBe),       // 荷兰语（弗拉芒语）
            "nl_nl" => Ok(Self::NlNl),       // 荷兰语
            "nn_no" => Ok(Self::NnNo),       // 挪威语（新挪威语）
            "no_no" => Ok(Self::NoNo),       // 挪威语（书面挪威语）
            "oc_fr" => Ok(Self::OcFr),       // 奥克语
            "ovd" => Ok(Self::Ovd),          // 埃尔夫达伦语
            "pl_pl" => Ok(Self::PlPl),       // 波兰语
            "pt_br" => Ok(Self::PtBr),       // 巴西葡萄牙语
            "pt_pt" => Ok(Self::PtPt),       // 欧洲葡萄牙语
            "qya_aa" => Ok(Self::QyaAa),     // 昆雅语（《魔戒》中的精灵语形式）
            "ro_ro" => Ok(Self::RoRo),       // 罗马尼亚语
            "rpr" => Ok(Self::Rpr),          // 俄语（革命前）
            "ru_ru" => Ok(Self::RuRu),       // 俄语
            "ry_ua" => Ok(Self::RyUa),       // 鲁辛语
            "sah_sah" => Ok(Self::SahSah),   // 雅库特语
            "se_no" => Ok(Self::SeNo),       // 北萨米语
            "sk_sk" => Ok(Self::SkSk),       // 斯洛伐克语
            "sl_si" => Ok(Self::SlSi),       // 斯洛文尼亚语
            "so_so" => Ok(Self::SoSo),       // 索马里语
            "sq_al" => Ok(Self::SqAl),       // 阿尔巴尼亚语
            "sr_cs" => Ok(Self::SrCs),       // 塞尔维亚语（拉丁文）
            "sr_sp" => Ok(Self::SrSp),       // 塞尔维亚语（西里尔文）
            "sv_se" => Ok(Self::SvSe),       // 瑞典语
            "sxu" => Ok(Self::Sxu),          // 上萨克森德语
            "szl" => Ok(Self::Szl),          // 西里西亚语
            "ta_in" => Ok(Self::TaIn),       // 泰米尔语
            "th_th" => Ok(Self::ThTh),       // 泰语
            "tl_ph" => Ok(Self::TlPh),       // 他加禄语
            "tlh_aa" => Ok(Self::TlhAa),     // 克林贡语
            "tok" => Ok(Self::Tok),          // 道本语（Toki Pona）
            "tr_tr" => Ok(Self::TrTr),       // 土耳其语
            "tt_ru" => Ok(Self::TtRu),       // 鞑靼语
            "uk_ua" => Ok(Self::UkUa),       // 乌克兰语
            "val_es" => Ok(Self::ValEs),     // 瓦伦西亚语
            "vec_it" => Ok(Self::VecIt),     // 威尼斯语
            "vi_vn" => Ok(Self::ViVn),       // 越南语
            "yi_de" => Ok(Self::YiDe),       // 意第绪语
            "yo_ng" => Ok(Self::YoNg),       // 约鲁巴语
            "zh_cn" => Ok(Self::ZhCn),       // 简体中文（中国大陆；普通话）
            "zh_hk" => Ok(Self::ZhHk),       // 繁体中文（中国香港；混合）
            "zh_tw" => Ok(Self::ZhTw),       // 繁体中文（中国台湾；普通话）
            "zlm_arab" => Ok(Self::ZlmArab), // 马来语（爪夷文）
            _ => Ok(Self::EnUs),             // 找不到时默认使用英语（美国）
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Locale, TRANSLATIONS, get_translation_text, reorder_substitutions};
    use crate::text::{TextComponentBase, TextContent, style::Style};

    fn arg(text: &str) -> TextComponentBase {
        TextComponentBase {
            content: Box::new(TextContent::Text {
                text: text.to_string().into(),
            }),
            style: Box::new(Style::default()),
            extra: vec![],
        }
    }

    /// `SubstitutionRange` 自述为横跨整个占位符，且
    /// 渲染器据此承诺从 `end + 1` 处恢复。单独的 `%` 占一个字节
    /// 宽，所以它不是占位符，也不得声明范围。
    #[test]
    fn a_lone_percent_is_not_a_placeholder() {
        let (substitutions, ranges) = reorder_substitutions("100% of %s", vec![arg("A")]);

        assert_eq!(ranges.len(), 1, "only the %s is a placeholder");
        assert_eq!(ranges[0].start, 8);
        assert_eq!(ranges[0].end, 9);
        assert_eq!(substitutions.len(), 1);
    }

    /// `assets/en_us_java.json` 将此内容原样作为 `attribute.modifier.plus.1` 提供。
    /// 它本身也会作为格式字符串传递给渲染器，因为
    /// 键未知时 `get_translation` 返回该键本身。
    #[test]
    fn an_escaped_percent_renders_as_one_percent() {
        assert_eq!(
            get_translation_text("+%s%% %s", Locale::EnUs, vec![arg("10"), arg("Speed")]),
            "+10% Speed"
        );
        // 随 `options.languageWarning` 一起发布。在这里以未知键传入，因此
        // `get_translation` 会将其转为小写后作为自身的格式串返回。
        assert_eq!(
            get_translation_text(
                "Language translations may not be 100%% accurate",
                Locale::EnUs,
                vec![arg("unused")]
            ),
            "language translations may not be 100% accurate"
        );
    }

    /// 以 `translation.test.escape` 形式提供，它是以……形式表述的转义规则
    /// 由游戏自身的翻译数据进行检验：`%%` 是一个字面 `%`，且只有
    /// 转义后剩下的 `%` 表示引入一次替换。
    #[test]
    fn runs_of_percents_escape_pairwise() {
        assert_eq!(
            get_translation_text(
                "%%s %%%s %%%%s %%%%%s",
                Locale::EnUs,
                vec![arg("A"), arg("B")]
            ),
            "%s %A %%s %%B"
        );
    }

    /// 调用者按其所处范围的位置来索引 `substitutions`，
    /// 因此两者必须始终保持一致。过去每当 `with` 被
    /// 与占位符数量不完全相等。
    #[test]
    fn a_substitution_is_produced_for_every_range() {
        for (translation, count) in [
            ("%s %s %s", 3),
            ("%s", 1),
            ("%1$s %1$s", 2),
            ("%% %s", 2),
            ("no placeholders", 0),
        ] {
            for args in 0..4 {
                let with = (0..args).map(|_| arg("x")).collect();
                let (substitutions, ranges) = reorder_substitutions(translation, with);

                assert_eq!(ranges.len(), count, "{translation:?} with {args} args");
                assert_eq!(
                    substitutions.len(),
                    ranges.len(),
                    "{translation:?} with {args} args"
                );
            }
        }
    }

    /// 以 `translation.test.invalid` 形式提供。
    #[test]
    fn a_trailing_percent_renders_instead_of_panicking() {
        assert_eq!(
            get_translation_text("hi %", Locale::EnUs, vec![arg("A")]),
            "hi %"
        );
    }

    /// 后面没有 `$s` 的数字也不是占位符，而且曾用于
    /// 使光标越出字符串末尾。
    #[test]
    fn trailing_digits_without_a_conversion_render_instead_of_panicking() {
        assert_eq!(
            get_translation_text("%5", Locale::EnUs, vec![arg("A")]),
            "%5"
        );
    }

    /// 渲染器必须比其附带的数据存活得更久。`en_us_java.json`
    /// 单独一项就持有 19 个字符串，过去足以拖垮整个服务器，因此需遍历
    /// 已加载的表并逐个渲染它们。
    #[test]
    fn every_shipped_translation_renders() {
        // 先收集：`get_translation` 会获取同一把锁。
        let keys: Vec<String> = {
            let translations = TRANSLATIONS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            translations[Locale::EnUs as usize]
                .keys()
                .cloned()
                .collect()
        };

        assert!(keys.len() > 1000, "the vanilla table should be loaded");
        for key in keys {
            // 非空的 `with` 才会使字符串走替换路径。
            get_translation_text(key, Locale::EnUs, vec![arg("A"), arg("B")]);
        }
    }

    /// 对照组：本模块所替换的形式必须保持原样。
    #[test]
    fn supported_placeholders_still_substitute() {
        assert_eq!(
            get_translation_text("%s joined", Locale::EnUs, vec![arg("Steve")]),
            "Steve joined"
        );
        assert_eq!(
            get_translation_text(
                "%1$s was slain by %2$s",
                Locale::EnUs,
                vec![arg("Steve"), arg("Alex")]
            ),
            "Steve was slain by Alex"
        );
    }

    /// 转换并不总是 `s`：解析器接受任何 ASCII 字母，因此
    /// `%d` 之类的数字形式同样会被替换。只读取 `s` 会留下
    /// 这些将被渲染为字面文本。
    #[test]
    fn a_numeric_conversion_substitutes_too() {
        assert_eq!(
            get_translation_text("+%d %s", Locale::EnUs, vec![arg("10"), arg("Speed")]),
            "+10 Speed"
        );
        assert_eq!(
            get_translation_text("%1$d blocks cloned", Locale::EnUs, vec![arg("42")]),
            "42 blocks cloned"
        );
    }

    /// 从未参与转换的数字不是占位符。渲染器
    /// 用于消耗其后的字符，把 `(%5 blocks away)` 变成
    /// `(Alocks away)`.
    #[test]
    fn digits_without_a_conversion_do_not_eat_the_next_character() {
        assert_eq!(
            get_translation_text("%1$s (%5 blocks away)", Locale::EnUs, vec![arg("Village")]),
            "Village (%5 blocks away)"
        );
        assert_eq!(
            get_translation_text("entered (%.2f)", Locale::EnUs, vec![arg("x")]),
            "entered (%.2f)"
        );
    }
}
