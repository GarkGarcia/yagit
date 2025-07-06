#![allow(clippy::borrow_interior_mutable_const, clippy::declare_interior_mutable_const)]
use std::{fmt::{self, Display}, mem, ffi::{CStr, CString}, sync::LazyLock};
use libc::{self, time_t, c_char};
use git2::Time;

const MINUTES_IN_AN_HOUR: u64 = 60;

const DATE_TIME_FMT: LazyLock<CString> = LazyLock::new(
  || CString::new("%Y-%m-%d %H:%M").unwrap()
);

const DATE_FMT: LazyLock<CString> = LazyLock::new(
  || CString::new("%d/%m/%Y %H:%M").unwrap()
);

const FULL_DATE_FMT: LazyLock<CString> = LazyLock::new(
  || CString::new("%a, %d %b %Y %H:%M:%S").unwrap()
);

#[derive(Clone, Copy, Debug)]
pub struct DateTime(pub Time);

#[derive(Clone, Copy, Debug)]
pub struct Date(pub Time);

#[derive(Clone, Copy, Debug)]
pub struct FullDate(pub Time);

const FTIME_BUFF_LEN:  usize = 64;
// TODO: [safety]: make this thread-safe?
// the application is currently single-threaded, so this is a non-issue for now
static mut FTIME_BUFF: [c_char; FTIME_BUFF_LEN] = [0; FTIME_BUFF_LEN];

#[allow(static_mut_refs)]
fn strftime(
  fmt: &CString,
  time: &Time,
  f: &mut fmt::Formatter<'_>
) -> fmt::Result {
  let time = time.seconds() as time_t;

  unsafe {
    let mut tm = mem::zeroed();
    libc::localtime_r(&time, &mut tm);

    libc::strftime(FTIME_BUFF.as_mut_ptr(), FTIME_BUFF_LEN, fmt.as_ptr(), &tm);
    FTIME_BUFF[FTIME_BUFF_LEN - 1] = 0; // prevent buffer overflows when
                                        // converting back to a CStr
    write!(f, "{}", CStr::from_ptr(FTIME_BUFF.as_ptr()).to_str().unwrap())
  }
}

impl Display for DateTime {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    strftime(&DATE_TIME_FMT, &self.0, f)
  }
}

impl Display for Date {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    strftime(&DATE_FMT, &self.0, f)
  }
}

impl Display for FullDate {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let timezone_sign = self.0.sign();
    let timezone_mins = self.0.offset_minutes().unsigned_abs() as u64;
    let timezone = timezone_mins / MINUTES_IN_AN_HOUR;

    strftime(&FULL_DATE_FMT, &self.0, f)?;
    write!(f, " {timezone_sign}{timezone:04}")
  }
}
