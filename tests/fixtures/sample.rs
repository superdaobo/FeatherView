// FeatherView 示例 Rust 文件
use serde::Serialize;

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

fn main() {
    let info = AppInfo {
        name: "featherview",
        version: "0.1.0",
    };
    println!("{:?}", serde_json::to_string(&info));
}
