//! Macros for logging.
//!
//! This implementation is NOT thread safe, since yagit is only expected to run
//! on my single-threaded server.
#![allow(static_mut_refs)]

use std::{io::{self, Write}, fmt::Arguments, time::Duration};

const BOLD_RED:    &str = "\u{001b}[1;31m";
const BOLD_GREEN:  &str = "\u{001b}[1;32m";
const BOLD_YELLOW: &str = "\u{001b}[1;33m";
const BOLD_BLUE:   &str = "\u{001b}[1;34m";
const BOLD_CYAN:   &str = "\u{001b}[1;36m";
const BOLD_WHITE:  &str = "\u{001b}[1;37m";
const UNDERLINE:   &str = "\u{001b}[4m";
const RESET:       &str = "\u{001b}[0m";

const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
static mut COUNTER: Counter = Counter {
  total: 0,
  count: 0,
  current_repo_name: String::new(),
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Level {
  Error,
  Info,
  Warn,
  Usage,
}

struct Counter {
  total:             usize,
  count:             usize,
  current_repo_name: String,
}

pub(crate) fn log(level: Level, args: &Arguments<'_>) {
  match level {
    Level::Error => {
      eprint!("{BOLD_RED}     Error{RESET} ");
      eprintln!("{}", args);
      // shouldn't print the job counter because we are about to die
    }
    Level::Info => {
      print!("{BOLD_BLUE}      Info{RESET} ");
      println!("{}", args);
      log_current_job();
    }
    Level::Warn => {
      print!("{BOLD_YELLOW}   Warning{RESET} ");
      println!("{}", args);
      log_current_job();
    }
    Level::Usage => {
      print!("{BOLD_YELLOW}     Usage{RESET} ");
      println!("{}", args);
      println!("          For more information check the {UNDERLINE}yagit(1){RESET} man page.");
      log_current_job();
    }
  }
}

pub(crate) fn query(args: &Arguments<'_>) -> String {
  let mut stdout = io::stdout();
  let stdin = io::stdin();
  let mut result = String::new();

  let _ = write!(stdout, "{BOLD_YELLOW}   Confirm{RESET} {} ", args);
  let _ = stdout.flush();

  if stdin.read_line(&mut result).is_err() {
    result.clear();
  } else if result.ends_with('\n') {
    let _ = result.pop();
  }

  // shouldn't print the job counter because we are should be running the
  // 'delete' command, so there are no jobs
  result
}

pub fn set_job_count(total: usize) {
  unsafe {
    COUNTER.total = total;
    COUNTER.count = 0;
  }
}

/// Logs a message telling the user the system has started rendering a job
pub fn render_start(repo_name: &str) {
  unsafe {
    COUNTER.count += 1;
    COUNTER.current_repo_name.clear();
    COUNTER.current_repo_name.push_str(repo_name);

    log_current_job();
  }
}

/// Logs a message telling the user the system has finished rendering a job
pub fn render_done() {
  unsafe {
    debug_assert!(COUNTER.count > 0);

    let space_padding = "... [/]".len() + 2 * crate::log_floor(COUNTER.total);
    println!(
      "{BOLD_GREEN}  Rendered{RESET} {name}{empty:space_padding$}",
      name  = COUNTER.current_repo_name,
      empty = "",
    );
  }
}

fn log_current_job() {
  unsafe {
    if COUNTER.count == 0 {
      return;
    }

    let mut stdout = io::stdout();

    let _ = write!(
      stdout,
      "{BOLD_CYAN} Rendering{RESET} {name}... {BOLD_WHITE}[{count:>padding$}/{total}]{RESET}\r",
      count = COUNTER.count,
      total = COUNTER.total,
      padding = crate::log_floor(COUNTER.total),
      name = COUNTER.current_repo_name,
    );
    let _ = stdout.flush();
  }
}

#[macro_export]
macro_rules! infoln {
  // infoln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Info,
      &std::format_args!($($arg)+),
    );
  });
}

#[macro_export]
macro_rules! errorln {
  // errorln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Error,
      &std::format_args!($($arg)+),
    );
  });
}

#[macro_export]
macro_rules! warnln {
  // warnln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Warn,
      &std::format_args!($($arg)+),
    );
  });
}

#[macro_export]
macro_rules! usageln {
  // usageln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Usage,
      &std::format_args!($($arg)+),
    );
  });
}

#[macro_export]
macro_rules! query {
  // query!("a {}", "question?");
  ($($arg:tt)+) => ({
    $crate::log::query(&std::format_args!($($arg)+))
  });
}

pub fn finished(duration: Duration) {
  let duration = duration.as_millis() / 100;
  let secs  = duration / 10;
  let dsecs = duration % 10;

  println!("{BOLD_GREEN}  Finished{RESET} Rendering took {secs}.{dsecs}s");
}

#[cfg(target_arch = "x86_64")]
pub fn version(program_name: &str) {
  if is_x86_feature_detected!("ssse3") {
    infoln!("Running {BOLD_WHITE}{program_name} {PROGRAM_VERSION}{RESET} (SIMD optimizations enabled)");
  } else {
    infoln!("Running {BOLD_WHITE}{program_name} {PROGRAM_VERSION}{RESET}");
  }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn version(program_name: &str) {
  infoln!("Running {BOLD_WHITE}{program_name} {PROGRAM_VERSION}{RESET}");
}
