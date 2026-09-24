use semver::Version;
use std::fs;
use wit_encoder::{Enum, Interface, Package, PackageName, TypeDef, TypeDefKind};

pub fn build() -> String {
    let particles: Vec<String> =
        serde_json::from_str(&fs::read_to_string("../../assets/particles.json").unwrap())
            .expect("解析 particles.json 失败");

    let mut package = Package::new(PackageName::new(
        "papokin",
        "plugin",
        Some(Version::new(0, 1, 0)),
    ));
    let mut interface = Interface::new("particles");

    let mut particle_enum = Enum::empty();
    for particle in particles {
        // WIT 的变体通常使用 kebab-case，但让我们看看是否应保持原样
        // Minecraft 粒子名已是 kebab-case 或下划线形式。
        // WIT 偏好 kebab-case。
        particle_enum.case(particle.replace('_', "-"));
    }

    interface.type_def(TypeDef::new("particle", TypeDefKind::Enum(particle_enum)));
    package.interface(interface);

    package.to_string()
}
