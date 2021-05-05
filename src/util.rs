use std::path::PathBuf;

pub fn parent_exe_dir() -> PathBuf {
    let mut dir = std::env::current_exe().expect("Binary directory unknown");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir
}
