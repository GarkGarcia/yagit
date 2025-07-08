use std::{
  io::{self, Read, Write},
  fs::{self, File},
  path::{Path, PathBuf},
  mem,
  env,
  fmt::{self, Display},
  ffi::OsStr,
  collections::HashMap,
  time::{Duration, SystemTime, Instant},
  process::ExitCode,
  os::unix::fs::PermissionsExt,
  cmp,
};
use git2::{
  Repository,
  Tree,
  Commit,
  ObjectType,
  Patch,
  Delta,
  DiffDelta,
  DiffLineType,
  Time,
  Oid,
  RepositoryInitOptions,
};

use time::{DateTime, Date, FullDate};
use command::{Cmd, SubCmd, Flags};
use config::{TREE_SUBDIR, BLOB_SUBDIR, COMMIT_SUBDIR};
use escape::Escaped;

#[cfg(not(debug_assertions))]
use std::borrow::Cow;

#[macro_use]
mod log;

mod escape;
mod markdown;
mod time;
mod command;
mod config;

const README_NAMES: &[&str] = &["README", "README.txt", "README.md"];
const LICENSE_NAME: &str    = "LICENSE";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageTitle<'a> {
  Index,
  Summary { repo_name: &'a str },
  Log { repo_name: &'a str },
  TreeEntry { repo_name: &'a str, path: &'a Path, },
  Commit { repo_name: &'a str, summary: &'a str },
  License { repo_name: &'a str },
}

struct RepoInfo {
  pub name:        String,
  pub owner:       String,
  pub description: Option<String>,

  pub repo:         Repository,
  pub last_commit:  Time,
  pub first_commit: u32,
}

impl RepoInfo {
  fn open<P, S>(path: P, name: S) -> Result<Self, ()>
  where
    P: AsRef<Path> + AsRef<OsStr> + fmt::Debug,
    S: AsRef<str>,
  {
    let repo = match Repository::open(&path) {
      Ok(repo) => repo,
      Err(_)   => {
        errorln!("Could not open repository in {path:?}");
        return Err(());
      }
    };

    let (first_commit, last_commit) = {
      let mut revwalk = repo.revwalk().unwrap();
      if let Err(e) = revwalk.push_head() {
        errorln!("Couldn't retrieve repository HEAD in {name:?}: {e}. Check if HEAD contains any commits and points to the right branch",
                 name = name.as_ref(),
                 e = e.message());
        return Err(());
      }

      revwalk.flatten().fold(
        (u32::MAX, Time::new(i64::MIN, 0)),
        |(min, max), commit_id| {
          let commit = repo.find_commit(commit_id).unwrap();
          let commit_time = commit.author().when();

          (
            cmp::min(min, commit_time.seconds() as u32),
            cmp::max_by(
              max,
              commit_time,
              |t1, t2| t1.seconds().cmp(&t2.seconds()),
            ),
          )
        }
      )
    };

    if first_commit == u32::MAX {
      errorln!("Repository {path:?} has no commits yet");
      return Err(());
    }

    let mut path = PathBuf::from(&path);
    if !repo.is_bare() {
      path.push(".git");
    }

    let owner = {
      let mut owner_path = path.clone();
      owner_path.push("owner");

      let mut owner = String::with_capacity(32);
      let read = File::open(owner_path)
        .map(|mut f| f.read_to_string(&mut owner));

      match read {
        Ok(Ok(_))  => owner,
        Ok(Err(e)) => {
          errorln!("Could not read the owner of {path:?}: {e}");
          return Err(());
        }
        Err(e) => {
          errorln!("Could not read the owner of {path:?}: {e}");
          return Err(());
        }
      }
    };

    let description = {
      let mut dsc_path = path.clone();
      dsc_path.push("description");
      let mut dsc = String::with_capacity(512);

      let read = File::open(dsc_path)
        .map(|mut f| f.read_to_string(&mut dsc));

      match read {
        Ok(Ok(_))  => Some(dsc),
        Ok(Err(e)) => {
          warnln!("Could not read the description of {path:?}: {e}");
          None
        }
        Err(e) => {
          warnln!("Could not read the description of {path:?}: {e}");
          None
        }
      }
    };

    Ok(Self {
      name: String::from(name.as_ref()),
      owner,
      description,
      repo,
      first_commit,
      last_commit,
    })
  }

  /// Returns an (orderer) index of the repositories in `config::REPOS_DIR` or
  /// `config::PRIVATE_REPOS_DIR`.
  fn index(private: bool) -> Result<Vec<Self>, ()> {
    let repos_dir = if private {
      config::PRIVATE_STORE_PATH
    } else {
      config::STORE_PATH
    };

    match fs::read_dir(repos_dir) {
      Ok(dir) => {
        let mut result = Vec::new();
        for entry in dir.flatten() {
          match entry.file_type() {
            Ok(ft) if ft.is_dir() => {
              let repo_path = entry.path();
              let repo_name = entry.file_name();

              result.push(
                RepoInfo::open(&repo_path, repo_name.to_string_lossy())?
              );
            }
            _ => continue,
          }
        }

        result.sort_by(|r1, r2| r2.first_commit.cmp(&r1.first_commit));

        Ok(result)
      }
      Err(e) => {
        errorln!("Could not read repositories in {repos_dir:?}: {e}");
        Err(())
      }
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadmeFormat {
  Txt,
  Md,
}

#[derive(Clone, Debug)]
struct Readme {
  content: String,
  path:    String,
  format:  ReadmeFormat,
}

struct RepoRenderer<'repo> {
  pub name:        &'repo str,
  pub description: Option<&'repo str>,

  pub repo:   &'repo Repository,
  pub head:   Tree<'repo>,
  pub branch: String,

  pub readme:  Option<Readme>,
  pub license: Option<String>,

  // cached constants which depend on command-line flags:
  // these shouldn't be modified at runtime
  pub full_build:  bool,
  pub output_path: PathBuf,
  pub output_root: &'static str,
}

impl<'repo> RepoRenderer<'repo> {
  fn new(repo: &'repo RepoInfo, flags: Flags) -> Result<Self, ()> {
    let (head, branch) = {
      match repo.repo.head() {
        Ok(head) => unsafe {
          let branch = head
            .shorthand()
            .expect("should be able to get HEAD shorthand")
            .to_string();

          let head = mem::transmute::<&Tree<'_>, &Tree<'repo>>(
            &head.peel_to_tree().unwrap()
          );

          (head.clone(), branch)
        }
        Err(e) => {
          errorln!("Could not retrieve HEAD of {name:?}: {e}",
                   name = repo.name);
          return Err(());
        }
      }
    };

    let mut readme = None;
    let mut license = None;
    for entry in head.iter() {
      if let (Some(ObjectType::Blob), Some(name)) =
             (entry.kind(), entry.name()) {
        if README_NAMES.contains(&name) {
          if let Some(Readme { path: ref old_path, .. }) = readme {
            warnln!("Multiple README files encountered: {old_path:?} and {name:?}. Ignoring {name:?}");
            continue;
          }

          let blob = entry
            .to_object(&repo.repo)
            .unwrap()
            .peel_to_blob()
            .unwrap();

          if blob.is_binary() {
            warnln!("README file {name:?} is binary. Ignoring {name:?}");
            continue;
          }

          let content = unsafe {
            // we trust Git to provide us valid UTF-8 on text files 
            std::str::from_utf8_unchecked(blob.content()).to_string()
          };

          let format = if name == "README.md" {
            ReadmeFormat::Md
          } else {
            ReadmeFormat::Txt
          };

          readme = Some(Readme { content, path: name.to_string(), format, });
        } else if name == LICENSE_NAME {
          let blob = entry
            .to_object(&repo.repo)
            .unwrap()
            .peel_to_blob()
            .unwrap();

          if blob.is_binary() {
            warnln!("LICENSE file is binary. Ignoring it");
            continue;
          }

          let content = unsafe {
            // we trust Git to provide us valid UTF-8 on text files 
            std::str::from_utf8_unchecked(blob.content()).to_string()
          };

          // TODO: [feature]: parse the license from content?
          license = Some(content);
        }
      }
    }

    let (output_path, output_root) = if flags.private() {
      let mut output_path = PathBuf::from(config::OUTPUT_PATH);
      output_path.push(config::PRIVATE_OUTPUT_ROOT);
      (output_path, config::PRIVATE_OUTPUT_ROOT)
    } else {
      (PathBuf::from(config::OUTPUT_PATH), "")
    };

    Ok(Self {
      name: &repo.name,
      description: repo.description.as_deref(),

      repo: &repo.repo,
      head,
      branch,

      readme,
      license,

      full_build: flags.full_build(),
      output_path,
      output_root,
    })
  }

  pub fn render(&self) -> io::Result<()> {
    self.render_summary()?;
    let last_commit_time = self.render_log()?;
    if let Some(ref license) = self.license {
      self.render_license(license)?;
    }
    self.render_tree(&last_commit_time)?;

    Ok(())
  }

  /// Prints the HTML preamble
  fn render_header(
    &self,
    f: &mut File,
    title: PageTitle<'repo>
  ) -> io::Result<()> {
    render_header(f, title)?;
    writeln!(f, "<main>")?;
    writeln!(f, "<h1>{title}</h1>", title = Escaped(self.name))?;
    if let Some(description) = self.description {
      writeln!(f, "<p>\n{d}\n</p>", d = Escaped(description.trim()))?;
    }
    writeln!(f, "<nav>")?;
    writeln!(f, "<ul>")?;
    writeln!(f, "<li{class}><a href=\"/{root}{name}/index.html\">summary</a></li>",
                root = self.output_root,
                name = Escaped(self.name),
                class = if matches!(title, PageTitle::Summary { .. }) { " class=\"nav-selected\"" } else { "" })?;
    writeln!(f, "<li{class}><a href=\"/{root}{name}/{COMMIT_SUBDIR}/index.html\">log</a></li>",
                root = self.output_root,
                name = Escaped(self.name),
                class = if matches!(title, PageTitle::Log { .. } | PageTitle::Commit { .. }) { " class=\"nav-selected\"" } else { "" })?;
    writeln!(f, "<li{class}><a href=\"/{root}{name}/{TREE_SUBDIR}/index.html\">tree</a></li>",
                root = self.output_root,
                name = Escaped(self.name),
                class = if matches!(title, PageTitle::TreeEntry { .. }) { " class=\"nav-selected\"" } else { "" })?;
    if self.license.is_some() {
      writeln!(f, "<li{class}><a href=\"/{root}{name}/license.html\">license</a></li>",
                  root = self.output_root,
                  name = Escaped(self.name),
                  class = if matches!(title, PageTitle::License { .. }) { " class=\"nav-selected\"" } else { "" })?;
    }
    writeln!(f, "</ul>")?;
    writeln!(f, "</nav>")
  }

  pub fn render_tree(
    &self,
    last_commit_time: &HashMap<Oid, SystemTime>,
  ) -> io::Result<()> {
    let mut tree_stack = Vec::new();
    let mut blob_stack = Vec::new();

    self.render_subtree(
      &self.head, PathBuf::new(), true,
      &mut tree_stack,
      &mut blob_stack,
    )?;

    while let Some((tree, path)) = tree_stack.pop() {
      self.render_subtree(
        &tree, path, false,
        &mut tree_stack,
        &mut blob_stack,
      )?;
    }

    for (blob, path) in blob_stack {
      self.render_blob(blob, path, last_commit_time)?;
    }

    Ok(())
  }

  fn render_subtree(
    &'repo self,
    tree: &Tree<'repo>,
    parent: PathBuf,
    root: bool,
    tree_stack: &mut Vec<(Tree<'repo>, PathBuf)>,
    blob_stack: &mut Vec<(Blob, PathBuf)>,
  ) -> io::Result<()> {
    let mut blobs_path = self.output_path.clone();
    blobs_path.push(self.name);
    blobs_path.push(BLOB_SUBDIR);
    blobs_path.extend(&parent);

    if !blobs_path.is_dir() {
      fs::create_dir(&blobs_path)?;
    }

    let mut index_path = self.output_path.clone();
    index_path.push(self.name);
    index_path.push(TREE_SUBDIR);
    index_path.extend(&parent);

    if !index_path.is_dir() {
      fs::create_dir(&index_path)?;
    }

    // ========================================================================
    index_path.push("index.html");

    let mut f = match File::create(&index_path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {index_path:?}: {e}");
        return Err(e);
      }
    };

    self.render_header(
      &mut f,
      PageTitle::TreeEntry { repo_name: self.name, path: &parent },
    )?;
    writeln!(&mut f, "<div class=\"table-container\">")?;
    writeln!(&mut f, "<table>")?;
    writeln!(&mut f, "<thead><tr><td>Name</td><tr></thead>")?;
    writeln!(&mut f, "<tbody>")?;

    if !root {
      writeln!(
        &mut f,
        "<tr><td><a href=\"..\" class=\"subtree\">..</a></td></tr>",
      )?;
    }

    // write the table rows
    for entry in tree.iter() {
      let name = entry.name().unwrap();
      let mut path = parent.clone();
      path.push(name);

      match entry.kind() {
        Some(ObjectType::Blob) => {
          writeln!(
            &mut f,
            "<tr><td><a href=\"/{root}{name}/{TREE_SUBDIR}/{path}.html\">{path}</a></td></tr>",
            root = self.output_root,
            name = Escaped(self.name),
            path = Escaped(&path.to_string_lossy()),
          )?;

          if name == "index" {
            warnln!("Blob named {path:?}! Skiping \"{}.html\"...",
                    path.to_string_lossy());
          } else {
            blob_stack.push(
              (Blob { id: entry.id(), mode: Mode(entry.filemode()) }, path)
            );
          }
        }
        Some(ObjectType::Tree) => {
          let subtree = entry
            .to_object(self.repo)
            .unwrap()
            .peel_to_tree()
            .unwrap();

          writeln!(
            &mut f,
            "<tr><td><a href=\"/{root}{name}/{TREE_SUBDIR}/{path}/index.html\" class=\"subtree\">{path}/</a></td></tr>",
            root = self.output_root,
            name = Escaped(self.name),
            path = Escaped(&path.to_string_lossy()),
          )?;

          tree_stack.push((subtree, path));
        }
        Some(ObjectType::Commit) => if !self.repo.is_bare() {
          let submod = self
            .repo
            .find_submodule(&path.to_string_lossy())
            .unwrap();

          if let Some(url) = submod.url() {
            writeln!(
              &mut f,
              "<tr><td><a href=\"{url}\" class=\"subtree\">{path}@</a></td></tr>",
              url = Escaped(url),
              path = Escaped(&path.to_string_lossy()),
            )?;
          } else {
            writeln!(
              &mut f,
              "<tr><td><span class=\"subtree\">{path}@</span></td></tr>",
              path = Escaped(&path.to_string_lossy()),
            )?;
          }
        } else {
          // we cannot lookup a submodule in a bare repo, because the
          // .gitmodules index is located in the working tree
          warnln!("Cannot lookup the {path:?} submodule in {repo}: {repo:?} is a bare repository",
                  repo = self.name);
          writeln!(
            &mut f,
            "<tr><td><span class=\"subtree\">{path}@</span></td></tr>",
            path = Escaped(&path.to_string_lossy()),
          )?;
        }
        Some(kind) => {
          unreachable!("unexpected tree entry kind {kind:?}")
        }
        None => unreachable!("couldn't get tree entry kind"),
      }
    }

    writeln!(&mut f, "</tbody>")?;
    writeln!(&mut f, "</table>")?;
    writeln!(&mut f, "</div>")?;

    writeln!(&mut f, "</main>")?;
    render_footer(&mut f)?;
    writeln!(&mut f, "</body>")?;
    writeln!(&mut f, "</html>")?;

    Ok(())
  }

  fn render_blob(
    &self,
    blob: Blob,
    path: PathBuf,
    last_commit_time: &HashMap<Oid, SystemTime>,
  ) -> io::Result<()> {
    let mut page_path = self.output_path.clone();
    page_path.push(self.name);
    page_path.push(TREE_SUBDIR);
    page_path.extend(&path);
    let page_path = format!("{}.html", page_path.to_string_lossy());

    // TODO: [optimize]: avoid late-stage decision-making by moving the 1st
    // `if` to outside of the function body?
    //
    // skip rendering the page if the commit the blob was last updated on is
    // older than the page
    if !self.full_build {
      if let Ok(meta) = fs::metadata(&page_path) {
        let last_modified = meta.modified().unwrap();
        if last_modified > last_commit_time[&blob.id] {
          return Ok(());
        }
      }
    }

    // ========================================================================
    let mode = blob.mode;
    let blob = self.repo
      .find_object(blob.id, None)
      .unwrap()
      .peel_to_blob()
      .unwrap();

    let mut raw_blob_path = self.output_path.clone();
    raw_blob_path.push(self.name);
    raw_blob_path.push(BLOB_SUBDIR);
    raw_blob_path.extend(&path);

    let mut blob_f = match File::create(&raw_blob_path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {raw_blob_path:?}: {e}");
        return Err(e);
      }
    };

    if let Err(e) = blob_f.write_all(blob.content()) {
      errorln!("Failed to copy file blob {raw_blob_path:?}: {e}");
      return Err(e);
    }

    let mut f = match File::create(&page_path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {page_path:?}: {e}");
        return Err(e);
      }
    };

    // ========================================================================
    self.render_header(
      &mut f,
      PageTitle::TreeEntry { repo_name: self.name, path: &path },
    )?;

    writeln!(&mut f, "<div class=\"table-container\">")?;
    writeln!(&mut f, "<table>")?;
    writeln!(&mut f, "<colgroup>")?;
    writeln!(&mut f, "<col />")?;
    writeln!(&mut f, "<col />")?;
    writeln!(&mut f, "<col style=\"width: 7em;\"/>")?;
    writeln!(&mut f, "</colgroup>")?;
    writeln!(&mut f, "<thead>")?;
    writeln!(&mut f, "<tr><td>Name</td><td align=\"right\">Size</td><td align=\"right\">Mode</td></tr>")?;
    writeln!(&mut f, "</thead>")?;
    writeln!(&mut f, "<tbody>")?;
    writeln!(&mut f, "<tr>")?;
    writeln!(&mut f, "<td><a href=\"./\" class=\"subtree\">..</a></td>")?;
    writeln!(&mut f, "<td align=\"right\"></td>")?;
    writeln!(&mut f, "<td align=\"right\"></td>")?;
    writeln!(&mut f, "</tr>")?;
    writeln!(&mut f, "<tr>")?;
    writeln!(&mut f, "<td><a href=\"/{root}{name}/{BLOB_SUBDIR}/{path}\">{path}</a></td>",
                     root = self.output_root,
                     name = Escaped(self.name),
                     path = Escaped(&path.to_string_lossy()))?;
    writeln!(&mut f, "<td align=\"right\">{}</td>", FileSize(blob.size()))?;
    writeln!(&mut f, "<td align=\"right\">{}</td>", mode)?;
    writeln!(&mut f, "</tr>")?;
    writeln!(&mut f, "</tbody>")?;
    writeln!(&mut f, "</table>")?;
    writeln!(&mut f, "</div>")?;

    if !blob.is_binary() && blob.size() > 0 {
      let content = unsafe {
        // we trust Git to provide us valid UTF-8 on text files 
        std::str::from_utf8_unchecked(blob.content())
      };
      let lines = content.matches('\n').count() + 1;
      let log_lines = log_floor(lines);

      writeln!(&mut f, "<div class=\"code-block blob\">")?;
      writeln!(&mut f, "<pre id=\"line-numbers\">")?;

      for n in 1..lines {
        writeln!(&mut f, "<a href=\"#l{n}\">{n:0log_lines$}</a>")?;
      }

      writeln!(&mut f, "</pre>")?;
      writeln!(&mut f, "<pre id=\"blob\">")?;

      for (i, line) in content.lines().enumerate() {
        writeln!(&mut f, "<span id=\"l{n}\">{line}</span>",
          line = Escaped(line), n = i + 1)?;
      }

      writeln!(&mut f, "</pre>")?;
      writeln!(&mut f, "</div>")?;
    }

    writeln!(&mut f, "</main>")?;
    render_footer(&mut f)?;
    writeln!(&mut f, "</body>")?;
    writeln!(&mut f, "</html>")?;

    Ok(())
  }

  fn render_log(&self) -> io::Result<HashMap<Oid, SystemTime>> {
    let mut last_mofied = HashMap::new();

    let mut revwalk = self.repo.revwalk().unwrap();
    revwalk.push_head().unwrap();
    let mut commits = Vec::new();

    for oid in revwalk.flatten() {
      let commit = self
        .repo
        .find_commit(oid)
        .expect("we should be able to find the commit");

      commits.push(commit);
    }

    // ========================================================================
    let mut index_path = self.output_path.clone();
    index_path.push(self.name);
    index_path.push(COMMIT_SUBDIR);

    if !index_path.is_dir() {
      fs::create_dir(&index_path)?;
    }

    index_path.push("index.html");

    let mut f = match File::create(&index_path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {index_path:?}: {e}");
        return Err(e);
      }
    };

    self.render_header(&mut f, PageTitle::Log { repo_name: self.name })?;
    writeln!(&mut f, "<div class=\"article-list\">")?;

    for commit in &commits {
      let commit_sig = commit.author();

      let author = commit_sig.name().unwrap();
      let time = commit_sig.when();
      let msg = commit
        .summary()
        .expect("commit summary should be valid UTF-8");

      let id = commit.id();

      // here there is some unnecessary allocation, but this is the best we can
      // do from within Rust because the Display implementation of git2::Oid
      // already allocates under the rug
      let shorthand_id = &format!("{}", id)[..8];

      writeln!(&mut f, "<article>")?;
      writeln!(&mut f, "<div>")?;
      writeln!(
        &mut f,
        "<span class=\"commit-heading\"><a href=\"/{root}{name}/{COMMIT_SUBDIR}/{id}.html\">{shorthand_id}</a> &mdash; {author}</span>",
        root = self.output_root,
        name = Escaped(self.name),
      )?;
      writeln!(&mut f, "<time datetime=\"{datetime}\">{date}</time>",
                       datetime  = DateTime(time), date = Date(time))?;
      writeln!(&mut f, "</div>")?;
      writeln!(&mut f, "<p>")?;
      writeln!(&mut f, "{msg}", )?;
      writeln!(&mut f, "</p>")?;
      writeln!(&mut f, "</article>")?;
    }

    writeln!(&mut f, "</div>")?;
    writeln!(&mut f, "</main>")?;
    render_footer(&mut f)?;
    writeln!(&mut f, "</body>")?;
    writeln!(&mut f, "</html>")?;

    for commit in commits {
      self.render_commit(&commit, &mut last_mofied)?;
    }

    Ok(last_mofied)
  }

  /// Renders the commit to HTML and updates the access time
  ///
  /// Shorcircutes if the commit page already exists.
  fn render_commit(
    &self,
    commit: &Commit<'repo>,
    last_commit_time: &mut HashMap<Oid, SystemTime>,
  ) -> io::Result<()> {
    let mut path = self.output_path.clone();
    path.push(self.name);
    path.push(COMMIT_SUBDIR);
    path.push(format!("{}.html", commit.id()));
    let should_skip = !self.full_build && path.exists();

    // ========================================================================
    #[derive(Debug)]
    struct DeltaInfo<'delta> {
      id: usize,

      add_count: usize,
      del_count: usize,
      delta:     DiffDelta<'delta>,

      new_path: &'delta Path,
      old_path: &'delta Path,

      num_hunks: usize,
      is_binary: bool,
    }

    let sig = commit.author();
    let time = sig.when();

    let diff = self
      .repo
      .diff_tree_to_tree(
        commit.parent(0).and_then(|p| p.tree()).ok().as_ref(),
        commit.tree().ok().as_ref(),
        None
      ).expect("diff between trees should be there");

    let deltas_iter = diff.deltas();
    let mut deltas: Vec<DeltaInfo<'_>> = Vec::with_capacity(deltas_iter.len());
    for (delta_id, diff_delta) in deltas_iter.enumerate() {
      // filter desired deltas
      if !matches!(diff_delta.status(),
                   Delta::Added | Delta::Copied | Delta::Deleted |
                   Delta::Modified | Delta::Renamed) {
        continue;
      }

      let old_file = diff_delta.old_file();
      let new_file = diff_delta.new_file();
      let old_path = &old_file.path().unwrap();
      let new_path = &new_file.path().unwrap();

      // collect the last time a file was modified at
      let id = new_file.id();
      let commit_time = Duration::from_secs(commit.time().seconds() as u64);
      let commit_time = SystemTime::UNIX_EPOCH + commit_time;
      if let Some(time) = last_commit_time.get_mut(&id) {
        // the newest time is NOT garanteed by
        // the order we loop through the commits
        if *time < commit_time {
          *time = commit_time;
        }
      } else {
        last_commit_time.insert(id, commit_time);
      }

      // TODO: [optmize]: refactor this? avoid late-stage decision making
      if should_skip {
        continue;
      }

      let patch = Patch::from_diff(&diff, delta_id)
        .unwrap()
        .expect("diff should have patch");

      let num_hunks = patch.num_hunks();

      let mut delta_info = DeltaInfo {
        id: delta_id,
        add_count: 0,
        del_count: 0,
        delta: diff_delta,
        old_path,
        new_path,
        num_hunks,
        is_binary: old_file.is_binary() || new_file.is_binary(),
      };

      for hunk_id in 0..num_hunks {
        let lines_of_hunk = patch
          .num_lines_in_hunk(hunk_id)
          .unwrap();

        for line_id in 0..lines_of_hunk { let line = patch
            .line_in_hunk(hunk_id, line_id)
            .unwrap();

          if line.old_lineno().is_none() {
            delta_info.add_count += 1;
          } else if line.new_lineno().is_none() {
            delta_info.del_count += 1;
          }
        }
      }

      deltas.push(delta_info);
    }

    // ========================================================================
    // skip rendering the commit page if the file already exists
    if should_skip {
      return Ok(());
    }

    // NOTE: this is an expensive operation, taking upwards of 76% of
    //       execution-time: Diff::stats should only be called when we
    //       know for the page needs updating
    let stats = diff.stats().expect("should be able to accumulate stats");

    let mut f = match File::create(&path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {path:?}: {e}");
        return Err(e);
      }
    };

    let summary = commit
      .summary()
      .expect("commit summary should be valid UTF-8");

    self.render_header(
      &mut f,
      PageTitle::Commit { repo_name: self.name, summary }
    )?;

    writeln!(&mut f, "<article class=\"commit\">")?;
    writeln!(&mut f, "<dl>")?;

    writeln!(&mut f, "<dt>Commit</dt>")?;
    writeln!(&mut f, "<dd><a href=\"/{root}{name}/{COMMIT_SUBDIR}/{id}.html\">{id}<a/><dd>",
                     root = self.output_root,
                     name = Escaped(self.name), id = commit.id())?;

    if let Ok(ref parent) = commit.parent(0) {
      writeln!(&mut f, "<dt>Parent</dt>")?;
      writeln!(
        &mut f,
        "<dd><a href=\"/{root}{name}/{COMMIT_SUBDIR}/{id}.html\">{id}<a/><dd>",
        root = self.output_root,
        name = Escaped(self.name),
        id = parent.id()
      )?;
    }

    writeln!(&mut f, "<dt>Author</dt>")?;
    write!(&mut f, "<dd>{name}", name = Escaped(sig.name().unwrap()))?;
    if let Some(email) = sig.email() {
      write!(&mut f, " &lt;<a href=\"mailto:{email}\">{email}</a>&gt;",
                     email = Escaped(email))?;
    }
    writeln!(&mut f, "</dd>")?;

    writeln!(&mut f, "<dt>Date</dt>")?;
    writeln!(&mut f, "<dd><time datetime=\"{datetime}\">{date}</time></dd>",
                     datetime = DateTime(time), date = FullDate(time))?;

    writeln!(&mut f, "</dl>")?;

    let message = commit
      .message()
      .expect("commit message should be valid UTF-8");
    for p in message.trim().split("\n\n") {
      writeln!(&mut f, "<p>\n{p}\n</p>", p = p.trim())?;
    }

    writeln!(&mut f, "</article>")?;

    // ========================================================================
    writeln!(&mut f, "<h2>Diffstats</h2>")?;
    writeln!(&mut f, "<p>{c} files changed, {i} insertions, {d} deletions</p>",
             c = stats.files_changed(),
             i = stats.insertions(),
             d = stats.deletions(),)?;

    writeln!(&mut f, "<div class=\"table-container\">")?;
    writeln!(&mut f, "<table>")?;
    writeln!(&mut f, "<thead>")?;
    writeln!(&mut f, "<tr>")?;
    writeln!(&mut f, "<td>Status</td>")?;
    writeln!(&mut f, "<td>Name</td>")?;
    writeln!(&mut f, "<td align=\"right\">Changes</td>")?;
    writeln!(&mut f, "<td align=\"right\">Insertions</td>")?;
    writeln!(&mut f, "<td align=\"right\">Deletions</td>")?;
    writeln!(&mut f, "<tr>")?;
    writeln!(&mut f, "</thead>")?;
    writeln!(&mut f, "<tbody>")?;

    for delta_info in &deltas {
      let delta_id = delta_info.id;

      writeln!(&mut f, "<tr>")?;

      write!(&mut f, "<td style=\"width: 4em;\">")?;
      match delta_info.delta.status() {
        Delta::Added    => write!(&mut f, "Added")?,
        Delta::Copied   => write!(&mut f, "Copied")?,
        Delta::Deleted  => write!(&mut f, "Deleted")?,
        Delta::Modified => write!(&mut f, "Modified")?,
        Delta::Renamed  => write!(&mut f, "Renamed")?,
        _               => unreachable!("other delta types should have been filtered out"),
      }
      writeln!(&mut f, "</td>")?;

      let old_file = delta_info.delta.old_file();
      let new_file = delta_info.delta.new_file();
      let old_path = old_file.path().unwrap().to_string_lossy();
      let new_path = new_file.path().unwrap().to_string_lossy();

      if old_path == new_path {
        writeln!(&mut f, "<td><a href=\"#d{delta_id}\">{old_path}</a></td>")?
      } else {
        writeln!(&mut f, "<td><a href=\"#d{delta_id}\">{old_path} &rarr; {new_path}</a></td>")?
      }

      match delta_info.delta.nfiles() {
        1 => writeln!(&mut f, "<td align=\"right\">1 file changed</td>")?,
        n => writeln!(&mut f, "<td align=\"right\">{n} files changed</td>")?,
      }
      writeln!(&mut f, "<td align=\"right\" style=\"width: 4em;\">{i}</td>",
                       i = delta_info.add_count)?;
      writeln!(&mut f, "<td align=\"right\" style=\"width: 4em;\">{d}</td>",
                       d = delta_info.del_count)?;
      writeln!(&mut f, "</tr>")?;
    }

    writeln!(&mut f, "</tbody>")?;
    writeln!(&mut f, "</table>")?;
    writeln!(&mut f, "</div>")?;

    // ========================================================================
    for delta_info in deltas {
      let delta_id = delta_info.id;

      writeln!(&mut f, "<div class=\"code-block\" id=\"d{delta_id}\">")?;

      match delta_info.delta.status() {
        Delta::Added => {
          writeln!(
            &mut f,
            "<pre><b>diff --git /dev/null b/<a href=\"/{root}{name}/{TREE_SUBDIR}/{new_path}.html\">{new_path}</a></b>",
            root = self.output_root,
            name = Escaped(self.name),
            new_path = delta_info.new_path.to_string_lossy(),
          )?;
        }
        Delta::Deleted => {
          writeln!(
            &mut f,
            "<pre><b>diff --git a/{old_path} /dev/null</b>",
            old_path = delta_info.old_path.to_string_lossy(),
          )?;
        }
        _ => {
          writeln!(
            &mut f,
            "<pre><b>diff --git a/<a id=\"d#{delta_id}\" href=\"/{root}{name}/{TREE_SUBDIR}/{new_path}.html\">{old_path}</a> b/<a href=\"/{root}{name}/{TREE_SUBDIR}/{new_path}.html\">{new_path}</a></b>",
            root = self.output_root,
            name = Escaped(self.name),
            new_path = delta_info.new_path.to_string_lossy(),
            old_path = delta_info.old_path.to_string_lossy(),
          )?;
        }
      }

      if delta_info.is_binary {
        writeln!(&mut f, "Binary files differ")?;
      } else {
        let patch = Patch::from_diff(&diff, delta_info.id)
          .unwrap()
          .expect("diff should have patch");

        for hunk_id in 0..delta_info.num_hunks {
          // we cannot cache the hunks:
          // libgit invalidates the data after a while
          let (hunk, lines_of_hunk) = patch.hunk(hunk_id).unwrap();

          write!(&mut f, "<a href=\"#d{delta_id}-{hunk_id}\" id=\"d{delta_id}-{hunk_id}\" class=\"h\">")?;
          f.write_all(hunk.header())?;
          write!(&mut f, "</a>")?;

          for line_id in 0..lines_of_hunk {
            let line = patch.line_in_hunk(hunk_id, line_id).unwrap();
            let line_content = unsafe {
              // we trust Git to provide us valid UTF-8 on text files 
              std::str::from_utf8_unchecked(line.content())
            };

            match delta_info.delta.status() {
              Delta::Modified => {
                let origin_type = line.origin_value();
                if matches!(origin_type,
                            DiffLineType::Addition | DiffLineType::Deletion) {
                  let (origin, class, lineno) = match origin_type {
                    DiffLineType::Addition => {
                      ('+', "i", line.new_lineno().unwrap())
                    }
                    DiffLineType::Deletion => {
                      ('-', "d", line.old_lineno().unwrap())
                    }
                    _ => unreachable!(),
                  };

                  write!(
                    &mut f,
                    "<a href=\"#d{delta_id}-{hunk_id}-{lineno}\" id=\"d{delta_id}-{hunk_id}-{lineno}\" class=\"{class}\">{origin}{line}</a>",
                    line = Escaped(line_content),
                  )?;
                } else {
                  write!(&mut f, " {line}", line = Escaped(line_content))?;
                }
              }
              Delta::Added => {
                write!(
                  &mut f,
                  "<a href=\"#d{delta_id}-{hunk_id}-{lineno}\" id=\"d{delta_id}-{hunk_id}-{lineno}\" class=\"i\">+{line}</a>",
                  lineno = line_id + 1,
                  line = Escaped(line_content),
                )?;
              }
              Delta::Deleted => {
                write!(
                  &mut f,
                  "<a href=\"#d{delta_id}-{hunk_id}-{lineno}\" id=\"d{delta_id}-{hunk_id}-{lineno}\" class=\"d\">-{line}</a>",
                  lineno = line_id + 1,
                  line = Escaped(line_content),
                )?;
              }
              _ => {},
            }
          }
        }
      }

      writeln!(&mut f, "</pre>")?;
      writeln!(&mut f, "</div>")?;
    }

    // ========================================================================
    writeln!(&mut f, "</main>")?;
    render_footer(&mut f)?;
    writeln!(&mut f, "</body>")?;
    writeln!(&mut f, "</html>")?;

    Ok(())
  }

  fn render_summary(&self) -> io::Result<()> {
    let mut path = self.output_path.clone();
    path.push(self.name);

    fs::create_dir_all(&path)?;
    path.push("index.html");

    let mut f = match File::create(&path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {path:?}: {e}");
        return Err(e);
      }
    };

    // ========================================================================
    self.render_header(&mut f, PageTitle::Summary { repo_name: self.name })?;

    writeln!(&mut f, "<ul>")?;
    writeln!(&mut f, "<li>refs: {branch}</li>",
                     branch = Escaped(&self.branch))?;
    writeln!(
      &mut f,
      "<li>git clone: <a href=\"git://git.pablopie.xyz/{name}\">git://git.pablopie.xyz/{name}</a></li>",
      name = Escaped(self.name),
    )?;
    writeln!(&mut f, "</ul>")?;

    if let Some(readme) = &self.readme {
      writeln!(&mut f, "<section id=\"readme\">")?;
      if readme.format == ReadmeFormat::Md {
        markdown::render_html(&mut f, &readme.content)?;
      } else {
        writeln!(&mut f, "<pre>{content}</pre>",
                         content = Escaped(&readme.content))?;
      }
      writeln!(&mut f, "</section>")?;
    }

    writeln!(&mut f, "</main>")?;
    render_footer(&mut f)?;
    writeln!(&mut f, "</body>")?;
    writeln!(&mut f, "</html>")?;

    Ok(())
  }

  pub fn render_license(&self, license: &str) -> io::Result<()> {
    let mut path = self.output_path.clone();
    path.push(self.name);
    path.push("license.html");

    let mut f = match File::create(&path) {
      Ok(f)  => f,
      Err(e) => {
        errorln!("Failed to create {path:?}: {e}");
        return Err(e);
      }
    };

    // ========================================================================
    self.render_header(&mut f, PageTitle::License { repo_name: self.name })?;
    writeln!(&mut f, "<section id=\"license\">")?;
    writeln!(&mut f, "<pre>{}</pre>", Escaped(license))?;
    writeln!(&mut f, "</section>")?;

    writeln!(&mut f, "</main>")?;
    render_footer(&mut f)?;
    writeln!(&mut f, "</body>")?;
    writeln!(&mut f, "</html>")?;

    Ok(())
  }
}

#[derive(Clone, Copy, Debug)]
struct Blob {
  id:   Oid,
  mode: Mode,
}

#[derive(Clone, Copy, Debug)]
/// POSIX filemode
struct Mode(pub i32);

impl Display for Mode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    const S_IFMT:   i32 = 0o170000; // file type mask
    const S_IFREG:  i32 = 0o100000; // regular file
    const S_IFDIR:  i32 = 0o040000; // directory
    const S_IFCHR:  i32 = 0o020000; // character device
    const S_IFBLK:  i32 = 0o060000; // block device
    const S_IFIFO:  i32 = 0o010000; // FIFO (named pipe)
    const S_IFLNK:  i32 = 0o120000; // symbolic link
    const S_IFSOCK: i32 = 0o140000; // socket
    const S_ISUID:  i32 = 0o4000;   // set-user-ID bit
    const S_ISGID:  i32 = 0o2000;   // set-group-ID bit
    const S_ISVTX:  i32 = 0o1000;   // sticky bit
    const S_IRUSR:  i32 = 0o4<<6;   // read permission for the owner
    const S_IWUSR:  i32 = 0o2<<6;   // write permission for the owner
    const S_IXUSR:  i32 = 0o1<<6;   // execute permission for the owner
    const S_IRGRP:  i32 = 0o4<<3;   // read permission for the group
    const S_IWGRP:  i32 = 0o2<<3;   // write permission for the group
    const S_IXGRP:  i32 = 0o1<<3;   // execute permission for the group
    const S_IROTH:  i32 = 0o4;      // read permission for others
    const S_IWOTH:  i32 = 0o2;      // write permission for others
    const S_IXOTH:  i32 = 0o1;      // execute permission for others

    let m = self.0;

    match m & S_IFMT { // filetype
      S_IFREG  => write!(f, "-")?,
      S_IFDIR  => write!(f, "d")?,
      S_IFCHR  => write!(f, "c")?,
      S_IFBLK  => write!(f, "b")?,
      S_IFIFO  => write!(f, "p")?,
      S_IFLNK  => write!(f, "l")?,
      S_IFSOCK => write!(f, "s")?,
      _        => write!(f, "?")?, // unknown type
    }

    if m & S_IRUSR != 0 { // owner read
      write!(f, "r")?;
    } else {
      write!(f, "-")?;
    }

    if m & S_IWUSR != 0 { // owner write
      write!(f, "w")?;
    } else {
      write!(f, "-")?;
    }

    match (m & S_ISUID != 0, m & S_IXUSR != 0) { // owner execute
      (true, true)   => write!(f, "s")?,
      (true, false)  => write!(f, "S")?,
      (false, true)  => write!(f, "x")?,
      (false, false) => write!(f, "-")?,
    }

    if m & S_IRGRP != 0 { // group read
      write!(f, "r")?;
    } else {
      write!(f, "-")?;
    }

    if m & S_IWGRP != 0 { // group write
      write!(f, "w")?;
    } else {
      write!(f, "-")?;
    }

    match (m & S_ISGID != 0, m & S_IXGRP != 0) { // group execute
      (true, true)   => write!(f, "s")?,
      (true, false)  => write!(f, "S")?,
      (false, true)  => write!(f, "x")?,
      (false, false) => write!(f, "-")?,
    }

    if m & S_IROTH != 0 { // others read
      write!(f, "r")?;
    } else {
      write!(f, "-")?;
    }

    if m & S_IWOTH != 0 { // others write
      write!(f, "w")?;
    } else {
      write!(f, "-")?;
    }

    match (m & S_ISVTX != 0, m & S_IXOTH != 0) { // others execute
      (true, true)   => write!(f, "t")?,
      (true, false)  => write!(f, "T")?,
      (false, true)  => write!(f, "x")?,
      (false, false) => write!(f, "-")?,
    }

    Ok(())
  }
}

#[derive(Clone, Copy, Debug)]
struct FileSize(usize);

impl Display for FileSize {
  // TODO: [feature]: print LOC instead of file size for text files?
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    const K: usize = 1000;
    const M: usize = K * 1000;

    let size = self.0;

    if size >= M {
      write!(f, "{}M", size/M)
    } else if size >= K {
      write!(f, "{}K", size/K)
    } else {
      write!(f, "{} bytes", size)
    }
  }
}

fn log_floor(n: usize) -> usize {
  if n == 0 {
    return 1;
  }

  let mut d = 0;
  let mut m = n;

  while m > 0 {
    d += 1;
    m /= 10;
  }

  d
}

fn render_header(f: &mut File, title: PageTitle<'_>) -> io::Result<()> {
  writeln!(f, "<!DOCTYPE html>")?;
  writeln!(f, "<html>")?;
  writeln!(f, "<head>")?;
  writeln!(f, "<meta http-equiv=\"Content-Type\" content=\"text/html; charset=UTF-8\"/>")?;
  writeln!(f, "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>")?;

  match title {
    PageTitle::Index => {
      writeln!(f, "<title>personal projects</title>")?;
    }
    PageTitle::Summary { repo_name }=> {
      writeln!(f, "<title>{repo}</title>", repo = Escaped(repo_name))?;
    }
    PageTitle::TreeEntry { repo_name, path } => {
      writeln!(f, "<title>/{path} at {repo}</title>",
                  repo = Escaped(repo_name),
                  path = Escaped(&path.to_string_lossy()))?;
    }
    PageTitle::Log { repo_name }=> {
      writeln!(f, "<title>{repo} log</title>", repo = Escaped(repo_name))?;
    }
    PageTitle::Commit { repo_name, summary } => {
      writeln!(f, "<title>{repo}: {summary}</title>",
                  repo = Escaped(repo_name),
                  summary = Escaped(summary.trim()))?;
    }
    PageTitle::License { repo_name } => {
      writeln!(f, "<title>{repo} license</title>", repo = Escaped(repo_name))?;
    }
  }

  writeln!(f, "<link rel=\"icon\" type=\"image/svg\" href=\"/favicon.svg\" />")?;
  writeln!(f, "<link rel=\"stylesheet\" type=\"text/css\" href=\"/styles.css\" />")?;
  writeln!(f, "</head>")?;
  writeln!(f, "<body>")?;
  writeln!(f, "<header>")?;
  writeln!(f, "<nav>")?;
  writeln!(f, "<img aria-hidden=\"true\" alt=\"Website logo\" src=\"/favicon.svg\">")?;
  writeln!(f, "<ul>")?;
  writeln!(f, "<li><strong><a href=\"https://pablopie.xyz\">pablo</a></strong></li>")?;
  writeln!(f, "<li><a href=\"/\">projects</a></li>")?;
  writeln!(f, "</ul>")?;
  writeln!(f, "</nav>")?;
  writeln!(f, "</header>")?;

  Ok(())
}

fn render_footer(f: &mut File) -> io::Result<()> {
  writeln!(f, "<footer>")?;
  writeln!(f, "made with ❤️ by <a rel=\"author\" href=\"https://pablopie.xyz/\">@pablo</a>")?;
  writeln!(f, "</footer>")
}

fn render_index(repos: &[RepoInfo], private: bool) -> io::Result<()> {
  let mut path = PathBuf::from(config::OUTPUT_PATH);
  if private {
    path.push(config::PRIVATE_OUTPUT_ROOT);
  }
  path.push("index.html");

  let output_root = if private {
    config::PRIVATE_OUTPUT_ROOT
  } else {
    ""
  };

  let mut f = match File::create(&path) {
    Ok(f)  => f,
    Err(e) => {
      errorln!("Failed to create {path:?}: {e}");
      return Err(e);
    }
  };

  // ==========================================================================
  render_header(&mut f, PageTitle::Index)?;
  writeln!(&mut f, "<main>")?;
  writeln!(&mut f, "<div class=\"article-list\">")?;

  for repo in repos {
    writeln!(&mut f, "<article>")?;

    writeln!(&mut f, "<h4>")?;
    writeln!(&mut f, "<a href=\"/{root}{repo}/index.html\">{repo}</a>",
                     root = output_root,
                     repo = Escaped(&repo.name))?;
    writeln!(&mut f, "</h4>")?;

    writeln!(&mut f, "<div>")?;
    writeln!(&mut f, "<span>{owner}</span>", owner = Escaped(&repo.owner))?;
    writeln!(&mut f, "<time datetime=\"{datetime}\">{date}</time>",
                     datetime  = DateTime(repo.last_commit),
                     date = Date(repo.last_commit))?;
    writeln!(&mut f, "</div>")?;

    if let Some(ref description) = repo.description {
      for p in description.trim().split("\n\n") {
        writeln!(&mut f, "<p>\n{p}\n</p>", p = p.trim())?;
      }
    }

    writeln!(&mut f, "</article>")?;
  }

  writeln!(&mut f, "</div>")?;
  writeln!(&mut f, "</main>")?;
  render_footer(&mut f)?;
  writeln!(&mut f, "</body>")?;
  writeln!(&mut f, "</html>")?;

  Ok(())
}

fn setup_repo(
  name: &str,
  path: &Path,
  description: &str,
  private: bool,
) -> io::Result<()> {
  let mut path = path.to_path_buf();
  path.push(".git");

  // ==========================================================================
  let mut owner_path = path.clone();
  owner_path.push("owner");

  let mut owner_f = match File::create(&owner_path) {
    Ok(f)  => f,
    Err(e) => {
      errorln!("Failed to create {owner_path:?}: {e}");
      return Err(e);
    }
  };

  write!(&mut owner_f, "{}", config::OWNER.trim())?;

  // ==========================================================================
  let mut dsc_path = path.clone();
  dsc_path.push("description");

  let mut dsc_f = match File::create(&dsc_path) {
    Ok(f)  => f,
    Err(e) => {
      errorln!("Failed to create {dsc_path:?}: {e}");
      return Err(e);
    }
  };

  write!(&mut dsc_f, "{}", description)?;

  // ==========================================================================
  let mut hook_path = path.clone();
  hook_path.push("hooks");
  hook_path.push("post-update");

  let mut hook_f = match File::create(&hook_path) {
    Ok(f)  => f,
    Err(e) => {
      errorln!("Failed to create {hook_path:?}: {e}");
      return Err(e);
    }
  };

  writeln!(&mut hook_f, "#!/bin/sh")?;
  if private {
    writeln!(&mut hook_f, "yagit --private render {name:?}")?;
  } else {
    writeln!(&mut hook_f, "yagit render {name:?}")?;
  }

  const HOOK_MODE: u32 = 0o755;
  let mut mode = hook_f.metadata()?.permissions();
  mode.set_mode(HOOK_MODE);

  drop(hook_f);
  if let Err(e) = fs::set_permissions(&hook_path, mode) {
    errorln!("Failed set permissions to {hook_path:?}: {e}");
    return Err(e);
  }

  // ==========================================================================
  // make it possible to push to the repo, eventhough it's not a bare repo
  let mut config_path = path;
  config_path.push("config");

  let mut config_opts = fs::OpenOptions::new();
  config_opts.append(true).create(true);

  let mut config_f = match config_opts.open(&config_path) {
    Ok(f)  => f,
    Err(e) => {
      errorln!("Failed to create {config_path:?}: {e}");
      return Err(e);
    }
  };

  writeln!(&mut config_f, "[receive]")?;
  writeln!(&mut config_f, "\tdenyCurrentBranch = updateInstead")?;

  Ok(())
}

#[cfg(not(debug_assertions))]
fn getuser<'a>() -> Cow<'a, str> {
  use std::ffi::CStr;

  unsafe {
    let uid = libc::getuid();
    let pw = libc::getpwuid(uid);
    assert!(!pw.is_null());

    CStr::from_ptr((*pw).pw_name).to_string_lossy()
  }
}

fn main() -> ExitCode {
  let mut args = env::args();
  let program_name = args.next().unwrap();

  let start = Instant::now();
  log::version(&program_name);

  let cmd = if let Ok(cmd) = Cmd::parse(&mut args, &program_name) {
    cmd
  } else {
    return ExitCode::FAILURE;
  };

  #[cfg(not(debug_assertions))]
  {
    use config::GIT_USER;

    let user = getuser();
    if user != GIT_USER {
      errorln!("Running {program_name} as the {user:?} user. Re-run as {GIT_USER:?}");
      return ExitCode::FAILURE;
    }
  }

  let repos_dir = if cmd.flags.private() {
    config::PRIVATE_STORE_PATH
  } else {
    config::STORE_PATH
  };

  match cmd.sub_cmd {
    SubCmd::RenderBatch => {
      let repos = if let Ok(repos) = RepoInfo::index(cmd.flags.private()) {
        repos
      } else {
        return ExitCode::FAILURE;
      };

      let n_repos = repos.len();
      infoln!("Updating pages for git repositories in {repos_dir:?}");
      log::set_job_count(n_repos+1); // tasks: render index + render each repo

      log::render_start("repository index");
      if render_index(&repos, cmd.flags.private()).is_err() {
        return ExitCode::FAILURE;
      }
      log::render_done();

      for repo in repos {
        let renderer = RepoRenderer::new(&repo, cmd.flags);
        let renderer = if let Ok(renderer) = renderer {
          renderer
        } else {
          return ExitCode::FAILURE;
        };

        log::render_start(&repo.name);
        if let Err(e) = renderer.render() {
          errorln!("Failed rendering pages for {name:?}: {e}",
                   name = renderer.name);
          return ExitCode::FAILURE;
        }
        log::render_done();
      }
    }
    SubCmd::Render { repo_name } => {
      let repos = if let Ok(repos) = RepoInfo::index(cmd.flags.private()) {
        repos
      } else {
        return ExitCode::FAILURE;
      };

      let mut repo = None;
      for r in &repos {
        if *r.name == *repo_name {
          repo = Some(r);
          break;
        }
      }

      if repo.is_none() {
        errorln!("Couldnt' find repository {repo_name:?} in {repos_dir:?}");
        return ExitCode::FAILURE;
      }
      let repo = repo.unwrap();

      let renderer = RepoRenderer::new(repo, cmd.flags);
      let renderer = if let Ok(renderer) = renderer {
        renderer
      } else {
        return ExitCode::FAILURE;
      };

      infoln!("Updating pages for git repository {repo_name:?}");
      log::set_job_count(2); // tasks: render index + render repo

      log::render_start("repository index");
      if let Err(e) = render_index(&repos, cmd.flags.private()) {
        errorln!("Failed rendering global repository index: {e}");
      }
      log::render_done();

      log::render_start(&repo.name);

      if let Err(e) = renderer.render() {
        errorln!("Failed rendering pages for {name:?}: {e}",
          name = renderer.name);
      }

      log::render_done();
    }
    SubCmd::Init { repo_name, description } => {
      let mut repo_path = if cmd.flags.private() {
        PathBuf::from(config::PRIVATE_STORE_PATH)
      } else {
        PathBuf::from(config::STORE_PATH)
      };
      repo_path.push(&repo_name);

      let mut opts = RepositoryInitOptions::new();
      opts.bare(false).no_reinit(true);

      infoln!("Initializing empty {repo_name:?} repository in {repo_path:?}");

      if let Err(e) = Repository::init_opts(&repo_path, &opts) {
        errorln!("Couldn't initialize {repo_name:?}: {e}", e = e.message());
        return ExitCode::FAILURE;
      }

      if setup_repo(&repo_name, &repo_path, &description, cmd.flags.private())
        .is_err() {
        return ExitCode::FAILURE;
      }
    }
  }

  log::finished(start.elapsed());
  ExitCode::SUCCESS
}
