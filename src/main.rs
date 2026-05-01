use std::env::home_dir;

use walkdir::WalkDir;

fn main() {
	traverse();
}

fn traverse() {
	let mut path = home_dir().unwrap();
	path.push("Music/Experiment");

	for item in WalkDir::new(&path)
		.into_iter()
		.filter_map(|res| res.ok())
	{
		println!("{:?}", item);
	}
}
