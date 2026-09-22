use anyhow::Result;

use crate::{RemoteArch, RemoteOs, RemotePlatform};

/// Parses the output of `uname -sm` to determine the remote platform.
/// Takes the last line to skip possible shell initialization output.
pub(crate) fn parse_platform(output: &str) -> Result<RemotePlatform> {
    let output = output.trim();
    let uname = output.rsplit_once('\n').map_or(output, |(_, last)| last);
    let Some((os, arch)) = uname.split_once(" ") else {
        anyhow::bail!("unknown uname: {uname:?}")
    };

    let os = match os {
        "Darwin" => RemoteOs::MacOs,
        "Linux" => RemoteOs::Linux,
        _ => anyhow::bail!(
            "Prebuilt remote servers are not yet available for {os:?}. See https://zed.dev/docs/remote-development"
        ),
    };

    // exclude armv5,6,7 as they are 32-bit.
    let arch = if arch.starts_with("armv8")
        || arch.starts_with("armv9")
        || arch.starts_with("arm64")
        || arch.starts_with("aarch64")
    {
        RemoteArch::Aarch64
    } else if arch.starts_with("x86") {
        RemoteArch::X86_64
    } else {
        anyhow::bail!(
            "Prebuilt remote servers are not yet available for {arch:?}. See https://zed.dev/docs/remote-development"
        )
    };

    Ok(RemotePlatform { os, arch })
}

/// The command (program + args) used to read a remote host's OS version, given
/// its detected OS.
///
/// The output is parsed by [`parse_os_version`].
pub(crate) fn os_version_command(os: RemoteOs) -> (&'static str, &'static [&'static str]) {
    match os {
        RemoteOs::Linux => ("cat", &["/etc/os-release"]),
        RemoteOs::MacOs => ("sw_vers", &["-productVersion"]),
        RemoteOs::Windows => ("cmd.exe", &["/c", "ver"]),
    }
}

/// Parses the output of [`os_version_command`] into a human-readable version
/// string, matching the conventions used by `client::telemetry::os_version`.
pub(crate) fn parse_os_version(os: RemoteOs, output: &str) -> Option<String> {
    let output = output.trim();
    if output.is_empty() {
        return None;
    }
    match os {
        RemoteOs::Linux => util::parse_os_release(output),
        RemoteOs::MacOs => output
            .lines()
            .next_back()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty()),
        RemoteOs::Windows => parse_windows_version(output),
    }
}

/// Extracts a `major.minor.build` version from the output of `cmd.exe /c ver`.
pub(crate) fn parse_windows_version(output: &str) -> Option<String> {
    output
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter_map(|token| {
            let parts: Vec<&str> = token.split('.').filter(|part| !part.is_empty()).collect();
            (parts.len() >= 3 && parts.iter().all(|part| part.parse::<u32>().is_ok()))
                .then(|| parts[..3].join("."))
        })
        .next()
}
