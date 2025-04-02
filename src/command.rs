use std::{env, ops::BitOrAssign};

const RENDER_BATCH_CMD: &str = "render-batch";
const RENDER_CMD:       &str = "render";
const INIT_CMD:         &str = "init";

const FULL_BUILD_FLAG: &str = "--full-build";
const PRIVATE_FLAG:    &str = "--private";

#[derive(Clone, Debug)]
pub struct Cmd {
  pub sub_cmd: SubCmd,
  pub flags:   Flags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CmdTag {
  RenderBatch,
  Render,
  Init,
}

#[derive(Clone, Debug)]
pub enum SubCmd {
  RenderBatch,
  Render {
    repo_name: String,
  },
  Init {
    repo_name:   String,
    description: String,
  }
}

impl Cmd {
  pub fn parse() -> Result<(Self, String), ()> {
    let mut args = env::args();
    let program_name = args.next().unwrap();

    let mut flags = Flags::EMPTY;
    let tag = loop {
      match args.next() {
        Some(arg) if arg == RENDER_BATCH_CMD => break CmdTag::RenderBatch,
        Some(arg) if arg == RENDER_CMD       => break CmdTag::Render,
        Some(arg) if arg == INIT_CMD         => break CmdTag::Init,

        Some(arg) if arg == FULL_BUILD_FLAG => {
          flags |= Flags::FULL_BUILD;
        }
        Some(arg) if arg == PRIVATE_FLAG => {
          flags |= Flags::PRIVATE;
        }

        Some(arg) if arg.starts_with("--") => {
          errorln!("Unknown flag {arg:?}");
          usage(&program_name, None);
          return Err(());
        }
        Some(arg) => {
          errorln!("Unknown subcommand {arg:?}");
          usage(&program_name, None);
          return Err(());
        }
        None => {
          errorln!("No subcommand provided");
          usage(&program_name, None);
          return Err(());
        }
      }
    };

    let sub_cmd = match tag {
      CmdTag::RenderBatch => {
        SubCmd::RenderBatch
      }
      CmdTag::Render => {
        let repo_name = if let Some(name) = args.next() {
          name
        } else {
          errorln!("No repository name providade");
          usage(&program_name, Some(tag));
          return Err(());
        };

        SubCmd::Render { repo_name, }
      }
      CmdTag::Init => {
        let repo_name = if let Some(name) = args.next() {
          name
        } else {
          errorln!("No repository name providade");
          usage(&program_name, Some(tag));
          return Err(());
        };

        let description = if let Some(dsc) = args.next() {
          dsc
        } else {
          errorln!("No description providade");
          usage(&program_name, Some(tag));
          return Err(());
        };

        SubCmd::Init { repo_name, description, }
      }
    };

    if args.next().is_some() {
      warnln!("Additional command line arguments provided. Ignoring trailing arguments...");
      usage(&program_name, Some(tag));
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

fn usage(program_name: &str, tag: Option<CmdTag>) {
  match tag {
    None => {
      usageln!("{program_name} [{FULL_BUILD_FLAG}] [{PRIVATE_FLAG}] <command> [<args>]");
    }
    Some(CmdTag::RenderBatch) => {
      usageln!("{program_name} [{FULL_BUILD_FLAG}] [{PRIVATE_FLAG}] {RENDER_BATCH_CMD}");
    }
    Some(CmdTag::Render) => {
      usageln!("{program_name} [{FULL_BUILD_FLAG}] [{PRIVATE_FLAG}] {RENDER_CMD} <repo-name>");
    }
    Some(CmdTag::Init) => {
      usageln!("{program_name} [{PRIVATE_FLAG}] {INIT_CMD} <repo-name>");
    }
  }
}
