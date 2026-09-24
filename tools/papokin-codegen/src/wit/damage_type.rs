use std::fs;

pub fn build() -> String {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/damage_type");
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("缺少 damage_type 目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .map(|e| e.path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();

    let mut cases = String::new();
    for raw_name in names {
        let name = raw_name
            .strip_prefix("minecraft:")
            .unwrap_or(&raw_name)
            .replace('_', "-");
        cases.push_str(&format!("    {name},\n"));
    }

    // 枚举项由数据源生成；自定义伤害类型 API（damage-scaling 等
    // 与 damage-type-manager resource）是手补接口面，固化在此模板中，
    // 确保重新生成不会丢失插件可见的 API 契约。
    format!(
        r##"package papokin:plugin@0.1.0;

interface damage-types {{
  enum damage-type {{
{cases}  }}

  /// 伤害量何时随服务器难度缩放。
  enum damage-scaling {{
    /// 伤害量从不随难度缩放。
    never,
    /// 仅当伤害由生物类非玩家实体造成时，伤害量才缩放。
    when-caused-by-living-non-player,
    /// 伤害量总是随难度缩放。
    always,
  }}

  /// 伤害类型可选的受伤音效/视觉效果覆盖。
  enum damage-effects {{
    hurt,
    thorns,
    drowning,
    burning,
    poking,
    freezing,
  }}

  /// 伤害类型的死亡消息如何组成。
  enum death-message-type {{
    /// 标准死亡消息。
    %default,
    /// 摔落伤害风格的变体。
    fall-variants,
    /// 带点击事件的"intentional game design"（故意的游戏设计）消息。
    intentional-game-design,
  }}

  /// 表示插件注册的自定义伤害类型定义。
  record custom-damage-type {{
    /// 条目的带命名空间 id，如 "my_plugin:frost"。
    name: string,
    /// 死亡消息 id；死亡消息翻译为 `death.attack.<message-id>`。
    message-id: string,
    /// 伤害量何时随难度缩放。
    scaling: damage-scaling,
    /// 施加给受到此伤害的玩家的消耗度（exhaustion）。
    exhaustion: f32,
    /// 可选的受伤音效/视觉效果覆盖。
    effects: option<damage-effects>,
    /// 死亡消息如何组成。
    death-message-type: death-message-type,
  }}

  /// 用于注册和查询自定义伤害类型的全局管理器。
  ///
  /// 仅在服务器启动期间（插件加载时）可以注册；注册表一旦冻结，
  /// `register-damage-type` 即返回错误。
  resource damage-type-manager {{
    /// 向服务器注册一个新的自定义伤害类型。
    register-damage-type: func(damage-type: custom-damage-type) -> result<_, string>;

    /// 按带命名空间的名称（如 "my_plugin:frost"）获取已注册的自定义
    /// 伤害类型。
    get-damage-type: func(name: string) -> option<custom-damage-type>;

    /// 检查某个自定义伤害类型名称是否已注册。
    has-damage-type: func(name: string) -> bool;

    /// 返回所有已注册自定义伤害类型的名称，按注册顺序排列。
    get-all-custom-damage-type-names: func() -> list<string>;
  }}
}}
"##
    )
}
