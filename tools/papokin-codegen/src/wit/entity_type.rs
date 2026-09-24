use semver::Version;
use std::collections::BTreeMap;
use std::fs;
use wit_encoder::{Enum, Interface, Package, PackageName, TypeDef, TypeDefKind};

pub fn build() -> String {
    let json: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&fs::read_to_string("../../assets/entities.json").unwrap())
            .expect("解析 entities.json 失败");

    let mut package = Package::new(PackageName::new(
        "papokin",
        "plugin",
        Some(Version::new(0, 1, 0)),
    ));
    let mut interface = Interface::new("entity-types");

    let mut entity_type_enum = Enum::empty();
    for name in json.keys() {
        entity_type_enum.case(name.replace('_', "-"));
    }

    interface.type_def(TypeDef::new(
        "entity-type",
        TypeDefKind::Enum(entity_type_enum),
    ));
    package.interface(interface);

    package.to_string()
}
