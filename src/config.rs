//! Compile-time configuration keys
// TODO: [feature]: read this from a TOML file at build-time?

#[cfg(not(debug_assertions))]
pub const REPOS_DIR: &str = "/var/git/public";

#[cfg(debug_assertions)]
pub const REPOS_DIR: &str = "./test/public";


#[cfg(not(debug_assertions))]
pub const PRIVATE_REPOS_DIR: &str = "/var/git/private";

#[cfg(debug_assertions)]
pub const PRIVATE_REPOS_DIR: &str = "./test/private";


#[cfg(not(debug_assertions))]
pub const OUTPUT_PATH: &str = "/var/www/git";

#[cfg(debug_assertions)]
pub const OUTPUT_PATH: &str = "./site";

pub const PRIVATE_OUTPUT_ROOT: &str = "private/";

#[cfg(not(debug_assertions))]
pub const GIT_USER: &str = "git";

pub const OWNER: &str = "Pablo";

pub const TREE_SUBDIR:   &str = "tree";
pub const BLOB_SUBDIR:   &str = "blob";
pub const COMMIT_SUBDIR: &str = "commit";
