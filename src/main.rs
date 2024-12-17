use std::{env, process};
use util::todo_err;
use walkdir::{DirEntry, WalkDir};

mod util;

struct DirIterCfg {
    skip_hidden: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for DirIterCfg {
    fn default() -> Self {
        Self { skip_hidden: false }
    }
}

fn main() {
    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let iter = WalkDir::new(file_path).into_iter();
    for entry in iter
        .filter_entry(|entry| !dir_iter_cfg.skip_hidden || is_hidden(entry))
        .filter_map(|entry_res| match entry_res {
            Ok(entry) => Some(entry),
            Err(err) => {
                todo_err!(err);
                None
            }
        })
    {}
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}
