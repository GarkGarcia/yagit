//! Macros for logging.
//!
//! This implementation is NOT thread safe, since yagit is only expected to run
//! on my single-threaded server.
#![allow(static_mut_refs)]
use std::{io::{self, Write}, fmt::Arguments};

const BOLD_WHITE:  &str = "\u{001b}[1;37m";
const BOLD_BLUE:   &str = "\u{001b}[1;34m";
const BOLD_RED:    &str = "\u{001b}[1;31m";
const BOLD_YELLOW: &str = "\u{001b}[1;33m";
const UNDERLINE:   &str = "\u{001b}[4m";
const RESET:       &str = "\u{001b}[0m";

static mut NEEDS_NEWLINE: bool = false;
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

pub(crate) fn log(level: Level, args: &Arguments<'_>, newline: bool) {
  match level {
    Level::Error => unsafe {
      let mut stderr = io::stderr();

      if NEEDS_NEWLINE {
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
    Level::Info => unsafe {
      let mut stdout = io::stdout();

      if NEEDS_NEWLINE {
        let _ = writeln!(stdout);
      }

      let _ = write!(stdout, "{BOLD_BLUE}INFO:{RESET} ");
      if newline {
        let _ = writeln!(stdout, "{}", args);
        log_job_counter();
      } else {
        let _ = write!(stdout, "{}", args);
        let _ = stdout.flush();
      }
    }
    Level::Warn => unsafe {
      let mut stdout = io::stdout();

      if NEEDS_NEWLINE {
        let _ = writeln!(stdout);
      }

      let _ = write!(stdout, "{BOLD_YELLOW}WARNING:{RESET} ");
      if newline {
        let _ = writeln!(stdout, "{}", args);
        log_job_counter();
      } else {
        let _ = write!(stdout, "{}", args);
      }
      if !newline { let _ = stdout.flush(); }
    }
    Level::Usage => unsafe {
      let mut stdout = io::stdout();

      if NEEDS_NEWLINE {
        let _ = writeln!(stdout);
      }

      let _ = write!(stdout, "{BOLD_YELLOW}USAGE:{RESET} ");
      if newline {
        let _ = writeln!(stdout, "{}", args);
        let _ = writeln!(stdout, "       For more information check the {UNDERLINE}yagit{RESET} man page.");
        log_job_counter();
      } else {
        let _ = write!(stdout, "{}", args);
      }
      if !newline { let _ = stdout.flush(); }
    }
  }

  if !newline {
    unsafe {
      NEEDS_NEWLINE = true;
    }
  }
}

pub fn info_done(args: Option<&Arguments<'_>>) {
  let mut stdout = io::stdout();
  let _ = match args {
    Some(args) => {
      writeln!(stdout, " {}", args)
    }
    None => {
      writeln!(stdout, " done!")
    }
  };
  unsafe {
    NEEDS_NEWLINE = false;
  }
}

pub fn job_counter_start(total: usize) {
  unsafe {
    COUNTER.total = total;
  }
}

pub fn job_counter_increment(repo_name: &str) {
  unsafe {
    COUNTER.count += 1;
    COUNTER.current_repo_name.clear();
    COUNTER.current_repo_name.push_str(repo_name);

    log_job_counter();

    // deinit the counter when we reach the total
    if COUNTER.count == COUNTER.total {
      COUNTER.total = 0;
      COUNTER.count = 0;
    }
  }
}

fn log_job_counter() {
  unsafe {
    if COUNTER.count == 0 {
      return;
    }

    let mut stdout = io::stdout();

    let _ = write!(
      stdout,
      "{BOLD_BLUE}->{RESET} {BOLD_WHITE}{count:>padding$}/{total}{RESET} {name}...",
      count = COUNTER.count,
      total = COUNTER.total,
      padding = crate::log_floor(COUNTER.total),
      name = COUNTER.current_repo_name,
    );
    let _ = stdout.flush();
    NEEDS_NEWLINE = true;
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
  // info_done!();
  () => ({
    $crate::log::info_done(None);
  });

  // info_done!("terminator");
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
  // error!("a {} event", "log");
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
  // warnln!("a {} event", "log");
  ($($arg:tt)+) => ({
    $crate::log::log(
      $crate::log::Level::Warn,
      &std::format_args!($($arg)+),
      true,
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
      true,
    );
  });
}
