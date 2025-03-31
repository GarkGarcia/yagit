use std::{env, ops::BitOrAssign};
use crate::log;

#[derive(Clone, Debug)]
pub struct Cmd {
  pub sub_cmd: SubCmd,
  pub flags:   Flags,
}

#[derive(Clone, Debug)]
pub enum SubCmd {
  RenderBatch,
  Render {
    repo_name: String,
  },
}

impl Cmd {
  pub fn parse() -> Result<(Self, String), ()> {
    let mut args = env::args();

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CmdTag {
      RenderBatch,
      Render,
    }

    let program_name = args.next().unwrap();

    let mut flags = Flags::EMPTY;
    let cmd = loop {
      match args.next() {
        Some(arg) if arg == "render-batch" => break CmdTag::RenderBatch,
        Some(arg) if arg == "render"       => break CmdTag::Render,

        // TODO: documment these flags
        Some(arg) if arg == "--full-build" => {
          flags |= Flags::FULL_BUILD;
        }
        Some(arg) if arg == "--private" => {
          flags |= Flags::PRIVATE;
        }

        Some(arg) if arg.starts_with("--") => {
          errorln!("Unknown flag {arg:?}");
          log::usage(&program_name);
          return Err(());
        }
        Some(arg) => {
          errorln!("Unknown subcommand {arg:?}");
          log::usage(&program_name);
          return Err(());
        }
        None => {
          errorln!("No subcommand provided");
          log::usage(&program_name);
          return Err(());
        }
      }
    };

    let sub_cmd = match cmd {
      CmdTag::RenderBatch => {
        SubCmd::RenderBatch
      }
      CmdTag::Render => {
        let repo_name = if let Some(dir) = args.next() {
          dir
        } else {
          errorln!("No repository name providade");
          log::usage(&program_name);
          return Err(());
        };

        SubCmd::Render { repo_name, }
      }
    };

    if args.next().is_some() {
      warnln!("Additional command line arguments provided. Ignoring trailing arguments...");
      log::usage(&program_name);
    }

    Ok((Self { sub_cmd, flags, }, program_name))
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Flags(u8);

impl Flags {
  const FULL_BUILD_RAW: u8 = 0b00000001;
  const PRIVATE_RAW:    u8 = 0b00000010;

  pub const EMPTY:      Self = Self(0);
  pub const FULL_BUILD: Self = Self(Self ::FULL_BUILD_RAW);
  pub const PRIVATE:    Self = Self(Self ::PRIVATE_RAW);

  pub fn full_build(self) -> bool {
    self.0 & Self::FULL_BUILD_RAW != 0
  }

  pub fn private(self) -> bool {
    self.0 & Self::PRIVATE_RAW != 0
  }
}

impl BitOrAssign for Flags {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0;
  }
}
