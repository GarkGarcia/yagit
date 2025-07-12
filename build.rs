use std::{fs::File, io::Write};

static_toml::static_toml! {
  static CONFIG = include_toml!("config.toml");
}

const STORE_PATH:          &str = CONFIG.git.release.store_path;
const PRIVATE_STORE_PATH:  &str = CONFIG.git.release.private_store_path;
const OUTPUT_PATH:         &str = CONFIG.output.release.path;
const PRIVATE_OUTPUT_ROOT: &str = CONFIG.output.private_output_root;

const MAN_SRC: &str = include_str!("src/yagit.1");

fn main() {
  let man_src = MAN_SRC
    .replace("PRIVATE_STORE_PATH", PRIVATE_STORE_PATH)
    .replace("STORE_PATH", STORE_PATH)
    .replace("OUTPUT_PATH", OUTPUT_PATH)
    .replace("PRIVATE_OUTPUT_ROOT", PRIVATE_OUTPUT_ROOT);

  let mut man_page = File::create("yagit.1")
    .expect("Could not create \"yagit.1\"");

  write!(&mut man_page, "{}", man_src).expect("Could not write to \"yagit.1\"");
}
