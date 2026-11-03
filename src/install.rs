//! `tabbify install` / `uninstall` / `init`: managing the shell integration block in the rc file.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::ValueEnum;

const PRE_ZSH: &str = include_str!("../shell/pre.zsh");
const POST_ZSH: &str = include_str!("../shell/post.zsh");
const PRE_FISH: &str = include_str!("../shell/pre.fish");
const POST_FISH: &str = include_str!("../shell/post.fish");
const PRE_BASH: &str = include_str!("../shell/pre.bash");
const POST_BASH: &str = include_str!("../shell/post.bash");

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellKind {
    Fish,
    Zsh,
    Bash,
}

impl ShellKind {
    pub fn detect(shell: &str) -> Result<Self> {
        match Path::new(shell).file_name().and_then(|s| s.to_str()) {
            Some("fish") => Ok(Self::Fish),
            Some("bash") => Ok(Self::Bash),
            Some("zsh") => Ok(Self::Zsh),
            _ => anyhow::bail!("unsupported shell {shell:?}; specify fish, bash, or zsh"),
        }
    }

    pub fn current() -> Result<Self> {
        Self::detect(&std::env::var("SHELL").unwrap_or_default())
    }

    pub fn name(self) -> &'static str {
        match self {
            ShellKind::Fish => "fish",
            ShellKind::Zsh => "zsh",
            ShellKind::Bash => "bash",
        }
    }

    pub fn rc(self) -> PathBuf {
        match self {
            ShellKind::Fish => {
                fish_rc(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from), dirs::home_dir().unwrap_or_default())
            }
            ShellKind::Zsh => {
                let dir = std::env::var_os("ZDOTDIR").map(PathBuf::from).or_else(dirs::home_dir).unwrap_or_default();
                dir.join(".zshrc")
            }
            ShellKind::Bash => dirs::home_dir().unwrap_or_default().join(".bashrc"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Part {
    Pre,
    Post,
    All,
}

fn exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tabbify".into())
}

pub fn init_script(shell: ShellKind, part: Part) -> String {
    let (pre, post) = match shell {
        ShellKind::Fish => (PRE_FISH, POST_FISH),
        ShellKind::Zsh => (PRE_ZSH, POST_ZSH),
        ShellKind::Bash => (PRE_BASH, POST_BASH),
    };
    let mut s = String::new();
    if part != Part::Post {
        s.push_str(pre);
    }
    if part != Part::Pre {
        s.push_str(post);
    }
    s.replace("@TABBIFY_BIN@", &if matches!(shell, ShellKind::Fish) { fish_quote(&exe()) } else { exe() })
}

// Markers around the blocks `tabbify install` adds to the rc file.
pub const PRE_BEGIN: &str = "# >>> tabbify pre >>>";
const PRE_END: &str = "# <<< tabbify pre <<<";
pub const POST_BEGIN: &str = "# >>> tabbify post >>>";
const POST_END: &str = "# <<< tabbify post <<<";

fn strip_block(text: &str, begin: &str, end: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.split_inclusive('\n') {
        let t = line.trim_end();
        if t == begin {
            skipping = true;
        } else if t == end && skipping {
            skipping = false;
        } else if !skipping {
            out.push_str(line);
        }
    }
    out
}

/// A relative `$XDG_CONFIG_HOME` is ignored.
fn fish_rc(xdg: Option<PathBuf>, home: PathBuf) -> PathBuf {
    xdg.filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config")).join("fish/config.fish")
}

fn fish_quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn managed_body(old: &str) -> Result<String> {
    let mut expected_end = None;
    for line in old.lines().map(str::trim_end) {
        if let Some(end) = match line {
            PRE_BEGIN => Some(PRE_END),
            POST_BEGIN => Some(POST_END),
            _ => None,
        } {
            anyhow::ensure!(expected_end.is_none(), "nested tabbify configuration block");
            expected_end = Some(end);
        } else if line == PRE_END || line == POST_END {
            anyhow::ensure!(expected_end == Some(line), "unmatched tabbify configuration marker");
            expected_end = None;
        }
    }
    anyhow::ensure!(expected_end.is_none(), "unterminated tabbify configuration block");
    Ok(strip_block(&strip_block(old, PRE_BEGIN, PRE_END), POST_BEGIN, POST_END))
}

fn installed_text(shell: ShellKind, old: &str, bin: &str) -> Result<String> {
    let body = managed_body(old)?;
    let sh = shell.name();
    let command = |part| {
        if matches!(shell, ShellKind::Fish) {
            format!("eval ({} init fish {part} | string collect)", fish_quote(bin))
        } else {
            format!("eval \"$(\"{bin}\" init {sh} {part})\"")
        }
    };
    Ok(format!(
        "{PRE_BEGIN}\n{}\n{PRE_END}\n{body}{}{POST_BEGIN}\n{}\n{POST_END}\n",
        command("pre"),
        if body.is_empty() || body.ends_with('\n') { "" } else { "\n" },
        command("post")
    ))
}

/// Backs up the original rc file once, as `<rc>.tabbify-backup`.
fn install_at(shell: ShellKind, rc: &Path, bin: &str) -> Result<()> {
    let old = match std::fs::read_to_string(rc) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let new = installed_text(shell, &old, bin)?;
    std::fs::create_dir_all(rc.parent().context("configuration has no parent")?)?;
    let backup = rc.with_file_name(format!("{}.tabbify-backup", rc.file_name().unwrap().to_string_lossy()));
    if rc.exists() && !backup.exists() {
        std::fs::copy(rc, &backup)?;
    }
    std::fs::write(rc, new).with_context(|| format!("writing {}", rc.display()))
}

pub fn install(shell: ShellKind) -> Result<()> {
    let rc = shell.rc();
    install_at(shell, &rc, &exe())?;
    println!("tabbify: updated {}", rc.display());
    println!("tabbify: open a new terminal (or run `exec {}`) to start using it.", shell.name());
    Ok(())
}

pub fn uninstall(shell: ShellKind) -> Result<()> {
    let rc = shell.rc();
    let old = match std::fs::read_to_string(&rc) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    std::fs::write(&rc, managed_body(&old)?)?;
    println!("tabbify: removed from {}", rc.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn detection_paths_and_quoting() {
        assert!(matches!(ShellKind::detect("/usr/bin/fish").unwrap(), ShellKind::Fish));
        assert!(matches!(ShellKind::detect("bash").unwrap(), ShellKind::Bash));
        assert!(matches!(ShellKind::detect("/bin/zsh").unwrap(), ShellKind::Zsh));
        assert!(ShellKind::detect("/bin/notfish").is_err());
        assert_eq!(fish_rc(Some("/custom".into()), "/home/test".into()), PathBuf::from("/custom/fish/config.fish"));
        assert_eq!(
            fish_rc(Some("relative".into()), "/home/test".into()),
            PathBuf::from("/home/test/.config/fish/config.fish")
        );
        assert_eq!(fish_quote("a'b\\c"), "'a\\'b\\\\c'");
        for part in [Part::Pre, Part::Post, Part::All] {
            let script = init_script(ShellKind::Fish, part);
            assert!(!script.contains("@TABBIFY_BIN@"));
            assert!(!script.contains("$("));
        }
    }

    #[test]
    fn install_preserves_content_backup_and_permissions() {
        let dir = std::env::temp_dir().join(format!(
            "tabbify-install-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let rc = dir.join("fish/config.fish");
        install_at(ShellKind::Fish, &rc, "/tmp/a 'quoted' tabbify").unwrap();
        let first = std::fs::read_to_string(&rc).unwrap();
        install_at(ShellKind::Fish, &rc, "/tmp/a 'quoted' tabbify").unwrap();
        assert_eq!(first, std::fs::read_to_string(&rc).unwrap());
        let body = "# user config\nset -g thing 'hello'\n\n";
        std::fs::write(&rc, body).unwrap();
        let backup = rc.with_file_name("config.fish.tabbify-backup");
        let existing_backup = std::fs::read(&backup).unwrap();
        std::fs::set_permissions(&rc, std::fs::Permissions::from_mode(0o600)).unwrap();
        install_at(ShellKind::Fish, &rc, "/tmp/tabbify").unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), existing_backup);
        assert_eq!(managed_body(&std::fs::read_to_string(&rc).unwrap()).unwrap(), body);
        assert_eq!(std::fs::metadata(&rc).unwrap().permissions().mode() & 0o777, 0o600);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_blocks_are_rejected() {
        for body in [
            format!("{PRE_BEGIN}\nkeep this\n"),
            format!("{POST_END}\n"),
            format!("{PRE_BEGIN}  \nkeep this\n"),
            format!("{PRE_BEGIN}\n{POST_BEGIN}\n{PRE_END}\n{POST_END}\n"),
            format!("{PRE_BEGIN}\n{PRE_BEGIN}\n{PRE_END}\n"),
        ] {
            assert!(installed_text(ShellKind::Fish, &body, "tabbify").is_err());
        }
        assert_eq!(managed_body("ordinary content").unwrap(), "ordinary content");
    }
}
