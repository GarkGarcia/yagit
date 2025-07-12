//! Compile-time configuration keys

static_toml::static_toml! {
  static CONFIG = include_toml!("config.toml");
}

#[cfg(not(debug_assertions))]
pub const OUTPUT_PATH: &str = CONFIG.output.path;

#[cfg(debug_assertions)]
pub const OUTPUT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test/site");

pub const TREE_SUBDIR:         &str = CONFIG.output.tree_subdir;
pub const BLOB_SUBDIR:         &str = CONFIG.output.blob_subdir;
pub const COMMIT_SUBDIR:       &str = CONFIG.output.commit_subdir;
pub const PRIVATE_OUTPUT_ROOT: &str = CONFIG.output.private_output_root;

#[cfg(not(debug_assertions))]
pub const GIT_USER: &str = CONFIG.git.user;
pub const OWNER:    &str = CONFIG.git.store_owner;

#[cfg(debug_assertions)]
pub const STORE_PATH:         &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test/public");
#[cfg(debug_assertions)]
pub const PRIVATE_STORE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test/private");

#[cfg(not(debug_assertions))]
pub const STORE_PATH:         &str = CONFIG.git.store_path;
#[cfg(not(debug_assertions))]
pub const PRIVATE_STORE_PATH: &str = CONFIG.git.private_store_path;
