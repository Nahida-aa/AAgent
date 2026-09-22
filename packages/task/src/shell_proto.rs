use anyhow::Context as _;

use crate::{Shell, ShellKind};

pub fn shell_from_proto(proto: proto::Shell) -> anyhow::Result<Shell> {
    let shell_type = proto.shell_type.context("invalid shell type")?;
    let shell = match shell_type {
        proto::shell::ShellType::System(_) => Shell::System,
        proto::shell::ShellType::Program(program) => Shell::Program(program),
        proto::shell::ShellType::WithArguments(program) => Shell::WithArguments {
            program: program.program,
            args: program.args,
            title_override: None,
        },
    };
    Ok(shell)
}

pub fn shell_to_proto(shell: Shell) -> proto::Shell {
    let shell_type = match shell {
        Shell::System => proto::shell::ShellType::System(proto::System {}),
        Shell::Program(program) => proto::shell::ShellType::Program(program),
        Shell::WithArguments {
            program,
            args,
            title_override: _,
        } => proto::shell::ShellType::WithArguments(proto::shell::WithArguments { program, args }),
    };
    proto::Shell {
        shell_type: Some(shell_type),
    }
}
