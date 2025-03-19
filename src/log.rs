//! Macros for logging.
//!
//! This implementation should be thread safe (unlink an implementation using
//! `println!` and `eprintln!`) because access to the global stdout/stderr
//! handle is syncronized using a lock.
use std::{io::{self, Write}, fmt::Arguments};

pub const BOLD_GREEN: &str =  "\u{001b}[1;32m";
pub const BOLD_RED:   &str =  "\u{001b}[1;31m";
pub const BOLD_YELLOW: &str = "\u{001b}[1;33m";
pub const RESET:      &str =  "\u{001b}[0m";

#[derive(Clone, Copy, Debug)]
pub(crate) enum Level {
  Error,
  Info,
  Warn,
}

pub(crate) fn log(level: Level, args: &Arguments<'_>, newline: bool) {
  match level {
    Level::Error => {
      let mut stderr = io::stderr();
      let _ = write!(stderr, "{BOLD_RED}=> ERROR:{RESET} ");
      let _ = if newline {
        writeln!(stderr, "{}", args)
      } else {
        write!(stderr, "{}", args)
      };
      if !newline { let _ = stderr.flush(); }
    }
    Level::Info => {
      let mut stdout = io::stdout().lock();
      let _ = write!(stdout, "{BOLD_GREEN}=>{RESET} ");
      let _ = if newline {
        writeln!(stdout, "{}", args)
      } else {
        write!(stdout, "{}", args)
      };
      if !newline { let _ = stdout.flush(); }
    }
    Level::Warn => {
      let mut stdout = io::stdout().lock();
      let _ = write!(stdout, "{BOLD_YELLOW}=> WARNING:{RESET} ");
      let _ = if newline {
        writeln!(stdout, "{}", args)
      } else {
        write!(stdout, "{}", args)
      };
      if !newline { let _ = stdout.flush(); }
    }
  }
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
    let _ = writeln!(io::stdout().lock(), " done!");
  });

  // infoln!("a {} event", "log");
  ($($arg:tt)+) => ({
    let _ = writeln!(
      io::stdout().lock(),
      " {}",
      &std::format_args!($($arg)+)
    );
  });
}

#[macro_export]
macro_rules! info_count {
  ($count:expr, $total:expr; $($arg:tt)+) => ({
    use crate::log::{BOLD_GREEN, RESET};
    let mut stdout = io::stdout().lock();
    let _ = write!(
      stdout,
      "{BOLD_GREEN}[{count:>padding$}/{total}]{RESET} {args}",
      count = $count,
      total = $total,
      padding = crate::log_floor($total),
      args = std::format_args!($($arg)+)
    );
    let _ = stdout.flush();
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
