use std::collections::BTreeMap;
use std::fs;

pub fn build() -> String {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/enchantment");
    let mut enchantment_vec: Vec<String> = fs::read_dir(dir)
        .expect("缺少附魔目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .map(|e| e.path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    enchantment_vec.sort();

    let mut cases = String::new();
    for raw_name in enchantment_vec {
        let name = raw_name.replace('_', "-");
        cases.push_str(&format!("    {name},\n"));
    }

    format!(
        r##"package papokin:plugin@0.1.0;

interface enchantments {{
  use text.{{text-component}};

  /// 附魔生效的装备槽位。
  enum attribute-modifier-slot {{
    any,
    main-hand,
    off-hand,
    hand,
    feet,
    legs,
    chest,
    head,
    armor,
    body,
    saddle,
  }}

  /// 原版附魔枚举。
  enum enchantment {{
{cases}  }}

  /// 表示一个自定义附魔定义。
  record custom-enchantment {{
    /// 附魔的唯一标识符（如 "my_plugin:lifesteal"）。
    id: string,
    /// 附魔的描述或显示名。
    description: text-component,
    /// 附魔的最高等级（如 1..=10）。
    max-level: u32,
    /// 铁砧修复/合成的基础花费倍率。
    anvil-cost: u32,
    /// 适用物品的标签或物品模式（如 "#minecraft:enchantable/weapon"）。
    supported-items: string,
    /// 附魔的权重/稀有度（越高越常见，默认 5）。
    weight: u32,
    /// 此附魔生效的装备槽位。
    slots: list<attribute-modifier-slot>,
    /// 互斥/冲突附魔 ID 的列表。
    exclusive-set: list<string>,
  }}

  /// 用于注册和查询自定义附魔的全局管理器。
  resource enchantment-manager {{
    /// 向服务器注册一个新的自定义附魔。
    register-enchantment: func(enchantment: custom-enchantment) -> result<_, string>;

    /// 按 ID 获取附魔定义。
    get-enchantment: func(id: string) -> option<custom-enchantment>;

    /// 检查某个附魔 ID 是否已注册。
    has-enchantment: func(id: string) -> bool;

    /// 返回所有已注册的自定义附魔 ID。
    get-all-enchantment-ids: func() -> list<string>;
  }}
}}
"##
    )
}
