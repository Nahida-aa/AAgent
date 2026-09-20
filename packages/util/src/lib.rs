pub mod paths;
pub mod shell;
pub mod shell_builder;
use std::{
    borrow::Cow,
    cmp,
    ops::{Range, RangeInclusive},
};
pub mod markdown;
pub use shell::{Shell, ShellKind};
pub use shell_builder::ShellBuilder;
#[cfg(not(target_family = "wasm"))]
pub mod command;
#[cfg(not(target_family = "wasm"))]
pub mod process;
pub mod redact;
pub mod schemars;
pub mod serde;

/// Removes characters from the end of the string if its length is greater than `max_chars` and
/// appends "..." to the string. Returns string unchanged if its length is smaller than max_chars.
pub fn truncate_and_trailoff(s: &str, max_chars: usize) -> String {
    debug_assert!(max_chars >= 5);

    // If the string's byte length is <= max_chars, walking the string can be skipped since the
    // number of chars is <= the number of bytes.
    if s.len() <= max_chars {
        return s.to_string();
    }
    let truncation_ix = s.char_indices().map(|(i, _)| i).nth(max_chars);
    match truncation_ix {
        Some(index) => s[..index].to_string() + "…",
        _ => s.to_string(),
    }
}

/// Re-exports that back [`fs_embed!`] so a caller only needs to depend on `util`,
/// not on `rust_embed` directly (the macro's dependency is an implementation
/// detail). Hidden from the public API.
#[doc(hidden)]
pub mod __rust_embed {
    pub use rust_embed::{EmbeddedFile, Filenames, Metadata, RustEmbed, utils};
}

/// Backs the dev arm of [`fs_embed!`]'s `iter`: every file under the root-relative
/// directory that passes the same rust_embed include/exclude globs the
/// release derive uses, so the dev and release file sets are identical. Reuses
/// rust_embed's own matcher rather than reimplementing glob semantics.
#[cfg(all(debug_assertions, not(feature = "debug-embed")))]
#[doc(hidden)]
pub fn __fs_embed_iter(
    root_relative: &str,
    includes: &[&str],
    excludes: &[&str],
) -> impl Iterator<Item = std::borrow::Cow<'static, str>> + 'static {
    fs_embed_file_names(root_relative, includes, excludes)
        .into_iter()
        .map(std::borrow::Cow::Owned)
}

#[cfg(all(debug_assertions, not(feature = "debug-embed")))]
fn fs_embed_file_names(root_relative: &str, includes: &[&str], excludes: &[&str]) -> Vec<String> {
    let Some(root) = dev_repo_root().map(|root| root.join(root_relative)) else {
        return Vec::new();
    };
    let matcher = rust_embed::utils::PathMatcher::new(includes, excludes);
    rust_embed::utils::get_files(root.to_string_lossy().into_owned(), matcher)
        .map(|entry| entry.rel_path)
        .collect()
}

/// The checkout that produced this binary, resolved at runtime by walking up to
/// the first ancestor that contains a `.git` entry (a directory in a normal
/// clone, a file in a git worktree or submodule). Cargo and corgi both place
/// built binaries under `<repo>/target/<profile>/`, so the repository root is
/// always an ancestor of the executable.
///
/// Dev-only affordances use this instead of baking a build-time path into the
/// artifact: such a path points at the wrong checkout from any other worktree
/// and, under corgi, is rejected because artifacts must be checkout-independent
/// to be shared across worktrees.
///
/// The executable's launch path is tried first, then its canonical form, then
/// the working directory. In CI, `target/` (or the checkout root) can be a
/// symlink onto another volume, so canonicalizing the executable alone can walk
/// off the checkout and miss `.git`; the launch path and the test runner's cwd
/// (a crate dir under the checkout) stay inside it.
pub fn dev_repo_root() -> Option<&'static std::path::Path> {
    use std::path::PathBuf;
    static ROOT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = std::env::current_exe().ok();
        let candidates = [
            exe.clone(),
            exe.and_then(|exe| exe.canonicalize().ok()),
            std::env::current_dir().ok(),
        ];
        candidates.into_iter().flatten().find_map(|start| {
            Some(
                start
                    .ancestors()
                    .find(|dir| dir.join(".git").exists())?
                    .to_path_buf(),
            )
        })
    })
    .as_deref()
}

/// Backs the dev arm of [`fs_embed!`]'s `get`: reads a single file from the
/// checkout, returning `None` when the include/exclude globs filter it out so
/// `get` matches release's embedded set exactly.
#[cfg(all(debug_assertions, not(feature = "debug-embed")))]
#[doc(hidden)]
pub fn __fs_embed_get(
    root_relative: &str,
    file_path: &str,
    includes: &[&str],
    excludes: &[&str],
) -> Option<rust_embed::EmbeddedFile> {
    let matcher = rust_embed::utils::PathMatcher::new(includes, excludes);
    if !matcher.is_path_included(file_path) {
        return None;
    }
    let root = dev_repo_root()
        .expect("dev asset loading requires running from within the checkout")
        .join(root_relative);
    rust_embed::utils::read_file_from_fs(&root.join(file_path)).ok()
}

#[cfg(not(feature = "debug-embed"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __fs_embed {
    (
        $vis:vis struct $name:ident,
        crate_relative = $crate_relative:literal,
        root_relative = $root_relative:literal
        $(, include = [$($include:literal),* $(,)?])?
        $(, exclude = [$($exclude:literal),* $(,)?])?
        $(,)?
    ) => {
        // `crate_path` points the derive's generated code at util's re-export so
        // the caller needs no direct `rust_embed` dependency.
        #[cfg(not(debug_assertions))]
        #[derive($crate::__rust_embed::RustEmbed)]
        #[crate_path = "::util::__rust_embed"]
        #[folder = $crate_relative]
        $($(#[include = $include])*)?
        $($(#[exclude = $exclude])*)?
        $vis struct $name;

        #[cfg(debug_assertions)]
        $vis struct $name;

        // Mirror the derive's public surface: inherent `get`/`iter` (callable
        // without the trait in scope) plus the trait impl (for generic bounds
        // like `util::asset_str` and `handlebars::register_embed_templates`), so
        // the two arms are interchangeable at call sites.
        #[cfg(debug_assertions)]
        impl $name {
            pub fn get(
                file_path: &str,
            ) -> ::core::option::Option<$crate::__rust_embed::EmbeddedFile> {
                $crate::__fs_embed_get(
                    $root_relative,
                    file_path,
                    &[$($($include),*)?],
                    &[$($($exclude),*)?],
                )
            }

            pub fn iter(
            ) -> impl ::core::iter::Iterator<Item = ::std::borrow::Cow<'static, str>> + 'static
            {
                $crate::__fs_embed_iter(
                    $root_relative,
                    &[$($($include),*)?],
                    &[$($($exclude),*)?],
                )
            }
        }

        #[cfg(debug_assertions)]
        impl $crate::__rust_embed::RustEmbed for $name {
            fn get(
                file_path: &str,
            ) -> ::core::option::Option<$crate::__rust_embed::EmbeddedFile> {
                <$name>::get(file_path)
            }

            fn iter(
            ) -> impl ::core::iter::Iterator<Item = ::std::borrow::Cow<'static, str>> + 'static
            {
                $crate::__fs_embed_iter(
                    $root_relative,
                    &[$($($include),*)?],
                    &[$($($exclude),*)?],
                )
            }
        }
    };
}

/// A `rust_embed` asset source that embeds files in release builds. Dev builds
/// read from the checkout at runtime unless the `debug-embed` feature is enabled,
/// in which case they embed the files too.
///
/// It expands to one of these arms:
/// * Release (`not(debug_assertions)`): `#[derive(RustEmbed)]` embedding
///   `crate_relative` at build time, with the given `include`/`exclude` globs.
/// * Dev with `debug-embed`: the same compile-time embedding as release.
/// * Dev without `debug-embed`: a runtime filesystem source rooted at
///   `root_relative`; edits appear on the next launch without a rebuild.
///
/// Two paths are required because the arms resolve from different bases: the
/// derive reads `crate_relative` relative to the crate's `Cargo.toml`, while the
/// runtime dev arm resolves `root_relative` relative to the repository root via
/// [`dev_repo_root`]. Baking the build-time path into that arm would point at the
/// wrong checkout from another worktree and is rejected by corgi, whose sandbox
/// requires checkout-independent output.
///
/// ```ignore
/// util::fs_embed! {
///     pub struct Assets,
///     crate_relative = "../../assets",
///     root_relative = "assets",
///     include = ["fonts/**/*", "themes/**/*", "*.md"],
///     exclude = ["themes/src/*", "*.DS_Store"],
/// }
/// ```
#[macro_export]
macro_rules! fs_embed {
    ($($tokens:tt)*) => {
        $crate::__fs_embed!($($tokens)*);
    };
}

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
