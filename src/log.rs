//! Macros for logging.
//!
//! This implementation should be thread safe (unlike an implementation using
//! `println!` and `eprintln!`) because access to the global stdout/stderr
//! handle is syncronized using a lock.
#![allow(static_mut_refs)]
use std::{io::{self, Write}, fmt::Arguments, sync::RwLock};

const BOLD_WHITE:  &str = "\u{001b}[1;37m";
const BOLD_BLUE:   &str = "\u{001b}[1;34m";
const BOLD_RED:    &str = "\u{001b}[1;31m";
const BOLD_YELLOW: &str = "\u{001b}[1;33m";
const RESET:       &str = "\u{001b}[0m";

// TODO: [optimize]: make a thread-unsafe version of this under a compile flag
static NEEDS_NEWLINE: RwLock<bool> = RwLock::new(false);
static mut COUNTER_TOTAL: RwLock<usize> = RwLock::new(0);
static mut COUNTER_STATE: RwLock<Option<CounterState>> = RwLock::new(None);

#[derive(Clone, Copy, Debug)]
pub(crate) enum Level {
  Error,
  Info,
  Warn,
}

struct CounterState {
  count: usize,
  current_repo_name: String,
}

pub(crate) fn log(level: Level, args: &Arguments<'_>, newline: bool) {
  match level {
    Level::Error => {
      let mut stderr = io::stderr();

      if needs_newline() {
        let _ = writeln!(stderr);
      }

      let _ = write!(stderr, "{BOLD_RED}ERROR:{RESET} ");
      if newline {
        let _ = writeln!(stderr, "{}", args);
        // shouldn't print the job counter because we are about to die
      } else {
        let _ = write!(stderr, "{}", args);
        let _ = stderr.flush();
      }
    }
    Level::Info => {
      let mut stdout = io::stdout().lock();

      if needs_newline() {
        let _ = writeln!(stdout);
      }

      let _ = write!(stdout, "{BOLD_BLUE}INFO:{RESET} ");
      if newline {
        let counter_state = unsafe { COUNTER_STATE.get_mut().unwrap() };
        let _ = writeln!(stdout, "{}", args);
        if let Some(ref counter) = counter_state {
          log_job_counter(counter);
        }
      } else {
        let _ = write!(stdout, "{}", args);
        let _ = stdout.flush();
      }
    }
    Level::Warn => {
      let mut stdout = io::stdout().lock();

      if needs_newline() {
        let _ = writeln!(stdout);
      }

      let _ = write!(stdout, "{BOLD_YELLOW}WARNING:{RESET} ");
      if newline {
        let counter_state = unsafe { COUNTER_STATE.get_mut().unwrap() };
        let _ = writeln!(stdout, "{}", args);
        if let Some(ref counter) = counter_state {
          log_job_counter(counter);
        }
      } else {
        let _ = write!(stdout, "{}", args);
      }
      if !newline { let _ = stdout.flush(); }
    }
  }

  if !newline {
    set_needs_newline(true);
  }
}

pub fn info_done(args: Option<&Arguments<'_>>) {
  let mut stdout = io::stdout().lock();
  let _ = match args {
    Some(args) => {
      writeln!(stdout, " {}", args)
    }
    None => {
      writeln!(stdout, " done!")
    }
  };
  set_needs_newline(false);
}

pub fn job_counter_start(total: usize) {
  unsafe {
    *COUNTER_TOTAL.write().unwrap() = total;
  }
}

pub fn job_counter_increment(repo_name: &str) {
  let counter_total = unsafe { COUNTER_TOTAL.get_mut().unwrap() };
  let counter = unsafe { COUNTER_STATE.get_mut().unwrap() };

  if let Some(ref mut inner) = counter {
    inner.count += 1;
    inner.current_repo_name = repo_name.to_owned();

    log_job_counter(inner);

    // deinit the counter when we reach the total
    if inner.count == *counter_total {
      *counter = None;
      *counter_total = 0;
    }
  } else {
    let new_counter = CounterState {
      count: 1,
      current_repo_name: repo_name.to_owned(),
    };

    log_job_counter(&new_counter);
    *counter = Some(new_counter);
  }
}

fn log_job_counter(counter: &CounterState) {
  let mut stdout = io::stdout().lock();
  let counter_total = unsafe { *COUNTER_TOTAL.read().unwrap() };

  let _ = write!(
    stdout,
    "{BOLD_BLUE}->{RESET} {BOLD_WHITE}{count:>padding$}/{total}{RESET} {name}...",
    count = counter.count,
    total = counter_total,
    padding = crate::log_floor(counter_total),
    name = counter.current_repo_name,
  );
  let _ = stdout.flush();
  set_needs_newline(true);
}

fn needs_newline() -> bool {
  *NEEDS_NEWLINE.read().unwrap()
}

fn set_needs_newline(val: bool) {
  *NEEDS_NEWLINE.write().unwrap() = val;
}

#[macro_export]
macro_rules! info {
  // info!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Info,
      &std::format_args!($($arg)+),
      false,
    );
  });
}

#[macro_export]
macro_rules! infoln {
  // infoln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Info,
      &std::format_args!($($arg)+),
      true,
    );
  });
}

#[macro_export]
macro_rules! info_done {
  () => ({
    $crate::log::info_done(None);
  });

  // infoln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::info_done(Some(&std::format_args!($($arg)+)));
  });
}

#[macro_export]
macro_rules! job_counter_start {
  ($total:expr) => ({
    $crate::log::job_counter_start($total as usize);
  });
}

#[macro_export]
macro_rules! job_counter_increment {
  ($repo_name:expr) => ({
    $crate::log::job_counter_increment(&$repo_name);
  });
}

#[macro_export]
macro_rules! error {
  // info!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Error,
      &std::format_args!($($arg)+),
      false,
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
      true,
    );
  });
}

#[macro_export]
macro_rules! warnln {
  // info!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Warn,
      &std::format_args!($($arg)+),
      true,
    );
  });
}

pub fn usage(program_name: &str) {
  let mut stderr = io::stderr();
  let _ = writeln!(
    stderr,
    r#"   {BOLD_YELLOW}USAGE:{RESET} {program_name} render       REPO_PATH  OUTPUT_PATH
          {program_name} render-batch BATCH_PATH OUTPUT_PATH"#
  );
}
