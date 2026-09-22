pub mod block;
pub mod item;

use std::fs;
use std::path::Path;

pub const SDK_OUT_DIR: &str = "../../crates/papokin-plugin-api/src/generated";

pub fn main() {
    fs::create_dir_all(SDK_OUT_DIR).expect("创建 SDK generated 目录失败");

    let targets: Vec<(fn() -> String, &str)> =
        vec![(item::build, "item.rs"), (block::build, "block.rs")];

    for (build_fn, file) in targets {
        println!("正在为 {} 生成 SDK", file);
        let code = build_fn();
        let path = Path::new(SDK_OUT_DIR).join(file);
        fs::write(&path, code).unwrap_or_else(|_| panic!("写入 {file} 失败"));
    }
}
