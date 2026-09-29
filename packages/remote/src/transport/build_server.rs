#[cfg(any(debug_assertions, feature = "build-remote-server-binary"))]
use crate::remote_client::RemoteOs;
#[cfg(any(debug_assertions, feature = "build-remote-server-binary"))]
use anyhow::{Context as _, Result};
#[cfg(any(debug_assertions, feature = "build-remote-server-binary"))]
use gpui::{AppContext as _, AsyncApp};

#[cfg(any(debug_assertions, feature = "build-remote-server-binary"))]
pub(crate) async fn build_remote_server_from_source(
    platform: &crate::RemotePlatform,
    delegate: &dyn crate::RemoteClientDelegate,
    binary_exists_on_server: bool,
    cx: &mut AsyncApp,
) -> Result<Option<std::path::PathBuf>> {
    use std::env::VarError;
    use util::command::{Command, Stdio, new_command};

    if let Ok(path) = std::env::var("ZED_COPY_REMOTE_SERVER") {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            return Ok(Some(path));
        } else {
            log::warn!(
                "ZED_COPY_REMOTE_SERVER path does not exist, falling back to ZED_BUILD_REMOTE_SERVER: {}",
                path.display()
            );
        }
    }

    // By default, we make building remote server from source opt-out and we do not force artifact compression
    // for quicker builds.
    let build_remote_server =
        std::env::var("ZED_BUILD_REMOTE_SERVER").unwrap_or("nocompress".into());

    if let "never" = &*build_remote_server {
        return Ok(None);
    } else if let "false" | "no" | "off" | "0" = &*build_remote_server {
        if binary_exists_on_server {
            return Ok(None);
        }
        log::warn!("ZED_BUILD_REMOTE_SERVER is disabled, but no server binary exists on the server")
    }

    async fn run_cmd(command: &mut Command) -> Result<()> {
        let output = command
            .kill_on_drop(true)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .await?;
        anyhow::ensure!(
            output.status.success(),
            "Failed to run command: {command:?}: output: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    async fn ensure_rustup_target(
        triple: &str,
        delegate: &dyn crate::RemoteClientDelegate,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        let rustup = which("rustup", cx)
            .await?
            .context("rustup not found on $PATH, install rustup (see https://rustup.rs/)")?;
        delegate.set_status(Some("Adding rustup target for cross-compilation"), cx);
        log::info!("adding rustup target");
        run_cmd(
            new_command(rustup)
                .current_dir(
                    util::dev_repo_root()
                        .context("locating the zed checkout to add the rustup target")?,
                )
                .args(["target", "add"])
                .arg(&triple),
        )
        .await?;
        Ok(())
    }

    enum RemoteServerBuildMode {
        Native,
        Xwin,
        Zig,
    }

    impl RemoteServerBuildMode {
        fn build_command(&self) -> &[&'static str] {
            match self {
                RemoteServerBuildMode::Native => &["build"],
                RemoteServerBuildMode::Xwin => &["xwin", "build"],
                RemoteServerBuildMode::Zig => &["zigbuild"],
            }
        }
    }

    let use_musl = !build_remote_server.contains("nomusl");
    let triple = format!(
        "{}-{}",
        platform.arch,
        match platform.os {
            RemoteOs::Linux =>
                if use_musl {
                    "unknown-linux-musl"
                } else {
                    "unknown-linux-gnu"
                },
            RemoteOs::MacOs => "apple-darwin",
            RemoteOs::Windows => "pc-windows-msvc",
        }
    );
    let mut rust_flags = match std::env::var("RUSTFLAGS") {
        Ok(val) => val,
        Err(VarError::NotPresent) => String::new(),
        Err(e) => {
            log::error!("Failed to get env var `RUSTFLAGS` value: {e}");
            String::new()
        }
    };
    if platform.os == RemoteOs::Linux && use_musl {
        rust_flags.push_str(" -C target-feature=+crt-static");

        if let Ok(path) = std::env::var("ZED_ZSTD_MUSL_LIB") {
            rust_flags.push_str(&format!(" -C link-arg=-L{path}"));
        }
    }
    let remote_build_mode = if platform.arch.as_str() == std::env::consts::ARCH
        && platform.os.as_str() == std::env::consts::OS
    {
        RemoteServerBuildMode::Native
    } else if platform.os.as_str() == "windows" {
        RemoteServerBuildMode::Xwin
    } else {
        RemoteServerBuildMode::Zig
    };

    match remote_build_mode {
        RemoteServerBuildMode::Native => {
            delegate.set_status(Some("Building remote server binary from source"), cx);
            log::info!("building remote server binary from source");
        }
        RemoteServerBuildMode::Zig => {
            if which("zig", cx).await?.is_none() {
                anyhow::bail!(if cfg!(not(windows)) {
                    "zig not found on $PATH, install zig (see https://ziglang.org/learn/getting-started or use zigup)"
                } else {
                    "zig not found on $PATH, install zig (use `winget install -e --id zig.zig` or see https://ziglang.org/learn/getting-started or use zigup)"
                });
            }

            ensure_rustup_target(&triple, delegate, cx).await?;

            if which("cargo-zigbuild", cx).await?.is_none() {
                delegate.set_status(Some("Installing cargo-zigbuild for cross-compilation"), cx);
                log::info!("installing cargo-zigbuild");
                run_cmd(new_command("cargo").args(["install", "--locked", "cargo-zigbuild"]))
                    .await?;
            }

            delegate.set_status(
                Some(&format!(
                    "Building remote binary from source for {triple} with Zig"
                )),
                cx,
            );
            log::info!("building remote binary from source for {triple} with Zig");
        }
        RemoteServerBuildMode::Xwin => {
            if which("clang", cx).await?.is_none() {
                anyhow::bail!(
                    "clang not found on $PATH, install clang to cross-compile the Windows remote server (see https://clang.llvm.org/)"
                );
            }

            if which("cargo-xwin", cx).await?.is_none() {
                anyhow::bail!(
                    "cargo-xwin not found on $PATH. Install it with `cargo install --locked cargo-xwin`.\n\n\
                     Note that cargo-xwin downloads Microsoft's CRT and Windows SDK; by using it you \
                     accept Microsoft's license (see https://go.microsoft.com/fwlink/?LinkId=2086102)"
                );
            }

            ensure_rustup_target(&triple, delegate, cx).await?;

            delegate.set_status(Some("Adding llvm-tools for cross-compilation"), cx);
            log::info!("adding llvm-tools component");
            run_cmd(
                new_command("rustup")
                    .current_dir(
                        util::dev_repo_root()
                            .context("locating the zed checkout to add the llvm-tools component")?,
                    )
                    .args(["component", "add", "llvm-tools"]),
            )
            .await?;

            delegate.set_status(
                Some(&format!(
                    "Building remote binary from source for {triple} with xwin"
                )),
                cx,
            );
            log::info!("building remote binary from source for {triple} with xwin");
        }
    };
    run_cmd(
        new_command("cargo")
            .current_dir(
                util::dev_repo_root()
                    .context("locating the zed checkout to build remote_server from source")?,
            )
            .args(remote_build_mode.build_command())
            .args([
                "--package",
                "remote_server",
                "--features",
                "debug-embed",
                "--target-dir",
                "target/remote_server",
                "--target",
                &triple,
            ])
            .env("RUSTFLAGS", &rust_flags),
    )
    .await?;
    let bin_path = util::dev_repo_root()
        .context("locating the zed checkout that built remote_server from source")?
        .join("target")
        .join("remote_server")
        .join(&triple)
        .join("debug")
        .join("remote_server")
        .with_extension(if platform.os.is_windows() { "exe" } else { "" });

    let path = if !build_remote_server.contains("nocompress") {
        delegate.set_status(Some("Compressing binary"), cx);

        #[cfg(not(target_os = "windows"))]
        let archive_path = {
            run_cmd(new_command("gzip").arg("-f").arg(&bin_path)).await?;
            bin_path.with_extension("gz")
        };

        #[cfg(target_os = "windows")]
        let archive_path = {
            let zip_path = bin_path.with_extension("zip");
            if smol::fs::metadata(&zip_path).await.is_ok() {
                smol::fs::remove_file(&zip_path).await?;
            }
            let compress_command = format!(
                "Compress-Archive -Path '{}' -DestinationPath '{}' -Force",
                bin_path.display(),
                zip_path.display(),
            );
            run_cmd(new_command("powershell.exe").args([
                "-NoProfile",
                "-Command",
                &compress_command,
            ]))
            .await?;
            zip_path
        };

        std::env::current_dir()?.join(archive_path)
    } else {
        bin_path
    };

    Ok(Some(path))
}

#[cfg(any(debug_assertions, feature = "build-remote-server-binary"))]
async fn which(
    binary_name: impl AsRef<str>,
    cx: &mut AsyncApp,
) -> Result<Option<std::path::PathBuf>> {
    let binary_name = binary_name.as_ref().to_string();
    let binary_name_cloned = binary_name.clone();
    let res = cx
        .background_spawn(async move { which::which(binary_name_cloned) })
        .await;
    match res {
        Ok(path) => Ok(Some(path)),
        Err(which::Error::CannotFindBinaryPath) => Ok(None),
        Err(err) => Err(anyhow::anyhow!(
            "Failed to run 'which' to find the binary '{binary_name}': {err}"
        )),
    }
}
