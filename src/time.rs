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

// TODO: [optimize]: allocation-free formatting?
//
// this is quite trick to implement by hand, so we would prolly have to row the
// (pretty heavy) chrono crate
//
// on the other hand, if we only render pages that need updating you don't
// expect to call this very often
fn strftime(
  fmt: &CString,
  time: &Time,
  f: &mut fmt::Formatter<'_>
) -> fmt::Result {
  let time = time.seconds() as time_t;
  unsafe {
    let mut tm = mem::zeroed();
    libc::localtime_r(&time, &mut tm);

    let mut buff: [c_char; 64] = [0; 64];
    libc::strftime(buff.as_mut_ptr(), buff.len(), fmt.as_ptr(), &tm);
    write!(f, "{}", CStr::from_ptr(buff.as_ptr()).to_str().unwrap())
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
