//! `tabbify update`: replaces the running binary with the latest GitHub release.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

fn run(cmd: &mut std::process::Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("running {:?}", cmd.get_program()))?;
    anyhow::ensure!(status.success(), "{:?} failed ({status})", cmd.get_program());
    Ok(())
}

/// A version as numbers, for comparing (`v1.0.10` > `1.0.9`).
fn version_key(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(['.', '-']).map_while(|p| p.parse().ok()).collect()
}

/// Installs the release build for this machine (static musl on Linux, native on macOS), after checking its sha256.
pub fn update(check: bool, force: bool) -> Result<()> {
    use std::process::Command;
    let current = env!("CARGO_PKG_VERSION");
    let repo = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/");
    let curl = |args: &[&str]| -> Result<String> {
        let out = Command::new("curl")
            .args(["-fsSL", "--connect-timeout", "10", "--max-time", "60"])
            .args(args)
            .output()
            .context("running curl (is it installed?)")?;
        anyhow::ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr).trim());
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };

    let release: serde_json::Value = serde_json::from_str(
        &curl(&[
            "-H",
            "Accept: application/vnd.github+json",
            &format!("https://api.github.com/repos/{repo}/releases/latest"),
        ])
        .context("looking up the latest release")?,
    )?;
    let tag = release["tag_name"].as_str().context("the latest release has no tag")?;
    let latest = tag.trim_start_matches('v');
    if version_key(latest) <= version_key(current) && !force {
        let note = if version_key(latest) < version_key(current) { " (newer than the latest release)" } else { "" };
        println!("tabbify: {current} is up to date{note}.");
        return Ok(());
    }
    if check {
        println!("tabbify: {latest} is available (you have {current}). Run `tabbify update` to install it.");
        return Ok(());
    }

    let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", a @ ("x86_64" | "aarch64")) => format!("{a}-unknown-linux-musl"),
        ("macos", a @ ("x86_64" | "aarch64")) => format!("{a}-apple-darwin"),
        (os, arch) => anyhow::bail!("there's no prebuilt tabbify for {arch}-{os}; build it from source instead"),
    };
    let name = format!("tabbify-{latest}-{triple}");
    let asset = |file: &str| {
        release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|a| a["name"] == file)
            .and_then(|a| a["browser_download_url"].as_str().map(str::to_string))
    };
    let archive = format!("{name}.tar.gz");
    let url = asset(&archive).with_context(|| format!("{tag} has no {archive}"))?;
    let sum_url = asset(&format!("{archive}.sha256")).with_context(|| format!("{tag} has no checksum"))?;

    // Next to the binary being replaced, so the final rename stays on one filesystem.
    let target = std::env::current_exe()?.canonicalize()?;
    let dir = target.parent().context("tabbify has no parent folder")?;
    let work = tempfile_dir(dir).with_context(|| {
        format!("can't write to {} (install the update with sudo, or as the owner of that folder)", dir.display())
    })?;
    let result = (|| -> Result<()> {
        println!("tabbify: downloading {archive}");
        run(Command::new("curl").args(["-fsSL", "--max-time", "300", "-o"]).arg(work.join(&archive)).arg(&url))?;
        std::fs::write(work.join(format!("{archive}.sha256")), curl(&[&sum_url])?)?;
        // macOS has no sha256sum; shasum ships with it.
        let (tool, flags): (_, &[&str]) =
            if cfg!(target_os = "macos") { ("shasum", &["-a", "256", "-s", "-c"]) } else { ("sha256sum", &["--quiet", "-c"]) };
        run(Command::new(tool).args(flags).arg(format!("{archive}.sha256")).current_dir(&work))
            .context("the download doesn't match its checksum")?;
        run(Command::new("tar").args(["xzf", &archive]).current_dir(&work))?;
        let new = work.join(&name).join("tabbify");
        let version = Command::new(&new).arg("--version").output().context("the new tabbify doesn't run")?;
        anyhow::ensure!(version.status.success(), "the new tabbify doesn't run");
        std::fs::rename(&new, &target).with_context(|| format!("replacing {}", target.display()))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result?;
    let what =
        if latest == current { format!("reinstalled {latest}") } else { format!("updated {current} → {latest}") };
    println!("tabbify: {what} ({}). Open a new terminal tab to use it (open tabs keep the old version).", target.display());
    Ok(())
}

fn tempfile_dir(parent: &Path) -> std::io::Result<PathBuf> {
    let dir = parent.join(format!(".tabbify-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_versions_compare_as_numbers() {
        assert!(version_key("v1.0.10") > version_key("1.0.9"));
        assert!(version_key("v1.1.0") > version_key("1.0.99"));
        assert_eq!(version_key("v1.0.5"), version_key("1.0.5"));
        assert!(version_key("2.0.0-rc1") > version_key("1.9.9"));
    }
}
