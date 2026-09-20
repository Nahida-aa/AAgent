pub mod paths;
pub mod shell;
pub mod shell_builder;
use std::{
    borrow::Cow,
    cmp,
    ops::{Range, RangeInclusive},
};

pub use shell::{Shell, ShellKind};
pub use shell_builder::ShellBuilder;
#[cfg(not(target_family = "wasm"))]
pub mod command;
#[cfg(not(target_family = "wasm"))]
pub mod process;
pub mod redact;
pub mod serde;

pub trait RangeExt<T> {
    fn sorted(&self) -> Self;
    fn to_inclusive(&self) -> RangeInclusive<T>;
    fn overlaps(&self, other: &Range<T>) -> bool;
    fn contains_inclusive(&self, other: &Range<T>) -> bool;
}

impl<T: Ord + Clone> RangeExt<T> for Range<T> {
    fn sorted(&self) -> Self {
        cmp::min(&self.start, &self.end).clone()..cmp::max(&self.start, &self.end).clone()
    }

    fn to_inclusive(&self) -> RangeInclusive<T> { self.start.clone()..=self.end.clone() }

    fn overlaps(&self, other: &Range<T>) -> bool {
        self.start < other.end && other.start < self.end
    }

    fn contains_inclusive(&self, other: &Range<T>) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

impl<T: Ord + Clone> RangeExt<T> for RangeInclusive<T> {
    fn sorted(&self) -> Self {
        cmp::min(self.start(), self.end()).clone()..=cmp::max(self.start(), self.end()).clone()
    }

    fn to_inclusive(&self) -> RangeInclusive<T> { self.clone() }

    fn overlaps(&self, other: &Range<T>) -> bool {
        self.start() < &other.end && &other.start <= self.end()
    }

    fn contains_inclusive(&self, other: &Range<T>) -> bool {
        self.start() <= &other.start && &other.end <= self.end()
    }
}

/// Configures the process to start a new session, to prevent interactive shells from taking control
/// of the terminal.
///
/// For more details: <https://registerspill.thorstenball.com/p/how-to-lose-control-of-your-shell>
pub fn set_pre_exec_to_start_new_session(
    command: &mut std::process::Command,
) -> &mut std::process::Command {
    // safety: code in pre_exec should be signal safe.
    // https://man7.org/linux/man-pages/man7/signal-safety.7.html
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    };
    command
}

/// Get an embedded file as a string.
pub fn asset_str<A: rust_embed::RustEmbed>(path: &str) -> Cow<'static, str> {
    match A::get(path).expect(path).data {
        Cow::Borrowed(bytes) => Cow::Borrowed(std::str::from_utf8(bytes).unwrap()),
        Cow::Owned(bytes) => Cow::Owned(String::from_utf8(bytes).unwrap()),
    }
}
