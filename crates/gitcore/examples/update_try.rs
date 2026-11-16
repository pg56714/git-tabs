//! Tries the updater by hand: `GIT_TABS_UPDATE_URL=http://host/latest.json cargo run -p gitcore
//! --example update_try -- <current version> <file to treat as the AppImage>`.
use gitcore::update::{Kind, apply_to, check};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (cur, target) = (args.get(1).map_or("0.0.1", String::as_str), args.get(2));
    match check(cur) {
        Ok(Some(rel)) => {
            println!("newer: {}", rel.version());
            if let Some(t) = target {
                println!("{:?}", apply_to(&rel, &Kind::AppImage(t.into())));
            }
        }
        Ok(None) => println!("up to date"),
        Err(e) => println!("check failed: {e}"),
    }
}
