use file::DirIterCfg;
use std::{env, process};
use walkdir::WalkDir;

mod file;
mod remote;
mod util;

fn main() {
    let dir_iter_cfg = DirIterCfg::default();

    let file_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("This program needs a path to scan");
        process::exit(1);
    });

    let iter = WalkDir::new(file_path).into_iter();
}
