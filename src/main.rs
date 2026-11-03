//! tabbify: Fig-style autocomplete for zsh, bash, and fish, in any terminal.
//! `tabbify shell` ([`session`]) runs the shell in a PTY and draws the popup; [`engine`] makes the suggestions.

mod config;
mod engine;
mod exec;
mod help;
mod icons;
mod install;
mod keys;
mod mirror;
mod osc;
mod parser;
mod popup;
mod procfs;
mod session;
mod spec;
mod specs;
mod store;
mod theme;
mod update;

use install::{POST_BEGIN, PRE_BEGIN, Part, ShellKind, init_script, install, uninstall};
use update::update;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "tabbify", version, about = "Fig-style IDE autocomplete for your terminal")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a shell inside tabbify (the shell integration does this for you)
    Shell {
        /// Shell to run (default: $SHELL)
        #[arg(long)]
        shell: Option<PathBuf>,
        /// Start it as a login shell
        #[arg(long)]
        login: bool,
        /// Spaces added by the zellij re-exec: room for the mirrored argv
        #[arg(hide = true, value_parser = parse_pad)]
        pad: Option<String>,
    },
    /// Print shell integration code for fish, zsh, or bash
    Init {
        #[arg(value_enum)]
        shell: ShellKind,
        /// `pre` goes at the top of the rc file, `post` at the bottom; `all` does both
        #[arg(value_enum, default_value_t = Part::All)]
        part: Part,
    },
    /// Add tabbify to your fish, zsh, or bash configuration (default: your $SHELL)
    Install {
        #[arg(value_enum)]
        shell: Option<ShellKind>,
    },
    /// Remove tabbify from your fish, zsh, or bash configuration (default: your $SHELL)
    Uninstall {
        #[arg(value_enum)]
        shell: Option<ShellKind>,
    },
    /// Manage completion specs
    Specs {
        #[command(subcommand)]
        cmd: SpecsCmd,
    },
    /// Print the suggestions for a command line (for debugging specs)
    Complete {
        /// Working directory to complete in
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Pretend this shell alias exists (`--alias gco='git checkout'`)
        #[arg(long = "alias", value_name = "NAME=VALUE")]
        aliases: Vec<String>,
        /// Text left of the cursor, e.g. "git checkout "
        line: String,
    },
    /// Check the installation
    Doctor,
    /// Update tabbify to the latest release on GitHub
    Update {
        /// Only check whether a newer release is available
        #[arg(long)]
        check: bool,
        /// Reinstall even if this is already the latest release
        #[arg(long)]
        force: bool,
    },
    /// List every icon (in the mode you use, next to its Unicode fallback)
    Icons,
    /// Show the name of each key you press, for the [keys] config
    Keys,
    /// Show a sample popup with your settings (or try others)
    Preview(PreviewArgs),
    /// Print the default configuration (save it to `tabbify config path`)
    Config {
        #[arg(value_enum, default_value_t = ConfigCmd::Default)]
        what: ConfigCmd,
    },
}

/// The hidden `tabbify shell` pad: only spaces, so `tabbify shell typo` is still an error.
fn parse_pad(s: &str) -> std::result::Result<String, String> {
    if !s.is_empty() && s.bytes().all(|b| b == b' ') {
        Ok(s.to_string())
    } else {
        Err(format!("unexpected argument '{s}'"))
    }
}

#[derive(Args)]
struct PreviewArgs {
    /// Theme: default, catppuccin, nord, dracula, gruvbox, tokyonight, light, terminal
    #[arg(long)]
    theme: Option<String>,
    /// rounded, square or none
    #[arg(long)]
    border: Option<String>,
    /// compact, normal or relaxed
    #[arg(long)]
    density: Option<String>,
    /// auto, nerd, unicode or none
    #[arg(long)]
    icons: Option<String>,
    /// auto, columns (60) or percent ("50%")
    #[arg(long)]
    width: Option<String>,
    /// rows (8) or percent ("30%")
    #[arg(long)]
    height: Option<String>,
    /// right or bottom
    #[arg(long)]
    description: Option<String>,
    /// Show every theme
    #[arg(long)]
    all: bool,
    /// Show every density
    #[arg(long)]
    densities: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ConfigCmd {
    Default,
    Path,
}

#[derive(Subcommand)]
enum SpecsCmd {
    /// List commands with a built-in or user spec
    List,
    /// Show where user specs and the --help cache live
    Path,
    /// Forget cached --help parsing results
    ClearCache,
    /// List the native generators spec files can use (`{"builtin": "<id>"}`)
    Generators,
}

/// Append a line to the debug log when `TABBIFY_LOG` is set.
pub fn log(msg: &str) {
    if std::env::var_os("TABBIFY_LOG").is_none() {
        return;
    }
    let dir = dirs::state_dir().unwrap_or_else(std::env::temp_dir).join("tabbify");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("tabbify.log")) {
        let _ = writeln!(f, "{msg}");
    }
}

/// `read` returns the tabbify shell's `/proc/<pid>/cmdline`, already polled.
fn mirror_status(tabbify_pid: Option<u32>, zellij: bool, no_mirror: bool, read: impl Fn() -> Option<Vec<u8>>) -> String {
    if !zellij {
        return "[off ] zellij mirror: not in zellij".into();
    }
    if no_mirror {
        return "[off ] zellij mirror: TABBIFY_NO_MIRROR set".into();
    }
    if tabbify_pid.is_none() {
        return "[off ] zellij mirror: not in a tabbify pane".into();
    }
    match tabbify_pid.and_then(|_| read()) {
        Some(c) if shows_doctor(&c) => format!("[ok  ] zellij mirror: on, argv area {} bytes", c.len()),
        _ => "[off ] zellij mirror: not mirroring (see TABBIFY_LOG)".into(),
    }
}

fn shows_doctor(cmdline: &[u8]) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let mut args = cmdline.split(|&b| b == 0);
    let tabbify =
        args.next().is_some_and(|a| Path::new(std::ffi::OsStr::from_bytes(a)).file_name() == Some("tabbify".as_ref()));
    tabbify && args.next() == Some(b"doctor")
}

/// Polls for up to 1.5 s, until the mirror shows `tabbify doctor`.
fn read_tabbify_cmdline(pid: u32) -> Option<Vec<u8>> {
    let mut last = None;
    for _ in 0..15 {
        last = std::fs::read(format!("/proc/{pid}/cmdline")).ok();
        if last.as_deref().is_some_and(shows_doctor) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    last
}

fn doctor() -> Result<()> {
    let ok = |b: bool| if b { "ok  " } else { "FAIL" };
    for shell in [ShellKind::Zsh, ShellKind::Bash, ShellKind::Fish] {
        let path = shell.rc();
        let rc = std::fs::read_to_string(&path).unwrap_or_default();
        let has = rc.contains(PRE_BEGIN) && rc.contains(POST_BEGIN);
        if has || ShellKind::current().is_ok_and(|current| shell.name() == current.name()) {
            println!("[{}] {} integration", ok(has), path.display());
        }
    }
    let tty = unsafe {
        let p = libc::ttyname(0);
        (!p.is_null()).then(|| std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
    };
    let inside = tty.is_some() && std::env::var("TABBIFY_TTY").ok() == tty;
    println!("[{}] this terminal is running inside tabbify", ok(inside));
    let cfg = config::config_path();
    if cfg.exists() {
        let valid =
            std::fs::read_to_string(&cfg).ok().and_then(|t| toml::from_str::<config::Config>(&t).ok()).is_some();
        println!("[{}] config {}", ok(valid), cfg.display());
    } else {
        println!("[ok  ] config: defaults ({} not present)", cfg.display());
    }
    let mut e = engine::Engine::new(store::Store::default(), config::Config::load().behavior);
    let env =
        engine::ShellEnv { cwd: std::env::current_dir()?, path: std::env::var("PATH").ok(), ..Default::default() };
    let n = e.complete("git ", &env, false, false).items.len();
    println!("[{}] engine: `git ` → {n} suggestions", ok(n > 0));
    let n = e.complete("ls --", &env, true, false).items.len();
    println!("[{}] --help parsing: `ls --` → {n} suggestions", ok(n > 0));
    let tabbify_pid = std::env::var("TABBIFY_PID").ok().and_then(|v| v.parse::<u32>().ok());
    println!(
        "{}",
        mirror_status(
            tabbify_pid,
            std::env::var_os("ZELLIJ").is_some(),
            std::env::var_os("TABBIFY_NO_MIRROR").is_some(),
            || tabbify_pid.and_then(read_tabbify_cmdline),
        )
    );
    Ok(())
}

fn complete(line: &str, cwd: Option<PathBuf>, aliases: Vec<String>) -> Result<()> {
    let cwd = match cwd {
        Some(c) => c,
        None => std::env::current_dir()?,
    };
    let mut e = engine::Engine::new(store::Store::default(), config::Config::load().behavior);
    let env = engine::ShellEnv {
        cwd,
        path: std::env::var("PATH").ok(),
        aliases: std::sync::Arc::new(
            aliases
                .into_iter()
                .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
                .collect(),
        ),
        ..Default::default()
    };
    let t = std::time::Instant::now();
    let c = e.complete(line, &env, true, true);
    let elapsed = t.elapsed();
    for s in &c.items {
        println!(
            "{:<11} {:<32} {:<28} {}",
            format!("{:?}", s.kind),
            s.label,
            format!("{:?}-{}", s.insert, s.delete),
            s.description
        );
    }
    eprintln!("{} suggestions in {:.1?}", c.items.len(), elapsed);
    Ok(())
}

fn list_specs() {
    let mut all: BTreeMap<String, &str> = BTreeMap::new();
    for c in specs::FIG.names() {
        all.insert(c.to_string(), "fig");
    }
    for c in specs::tabbify_names() {
        let src = if all.contains_key(c) { "tabbify + fig" } else { "tabbify" };
        all.insert(c.to_string(), src);
    }
    for c in store::Store::default().commands() {
        all.insert(c, "user");
    }
    for (c, src) in &all {
        println!("{c:<32} {src}");
    }
    eprintln!("{} commands ({} tabbify + {} fig spec files embedded)", all.len(), specs::TABBIFY.count(), specs::FIG.count());
}

fn list_icons() {
    icons::detect();
    let cfg = config::Config::load();
    icons::set_overrides(&cfg.icons);
    let style = cfg.ui.icons;
    let nerd = icons::use_nerd(&style);
    println!("icons = \"{style}\" → using {}\n", if nerd { "Nerd Font glyphs" } else { "Unicode symbols" });
    let fallback = icons::legend(false);
    for (i, (g, c, name, what)) in icons::legend(nerd).into_iter().enumerate() {
        let alt = fallback.get(i).filter(|_| nerd).map_or(String::new(), |(ug, ..)| format!("   (unicode: {ug})"));
        println!("  \x1b[{c}m{g}\x1b[0m  {name:<12} {what}{alt}");
    }
    println!("\nChange any of them in the [icons] section of {}.", config::config_path().display());
}

fn preview(args: PreviewArgs) {
    icons::detect();
    let cfg = config::Config::load();
    icons::set_overrides(&cfg.icons);
    let mut ui = cfg.ui;
    ui.theme = args.theme.unwrap_or(ui.theme);
    ui.border = args.border.unwrap_or(ui.border);
    ui.density = args.density.unwrap_or(ui.density);
    ui.icons = args.icons.unwrap_or(ui.icons);
    if let Some(w) = args.width {
        ui.width = config::Size::Text(w);
    }
    if let Some(h) = args.height {
        ui.height = config::Size::Text(h);
    }
    if let Some(d) = args.description {
        ui.description_position = d;
    }
    let cols = unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 { ws.ws_col } else { 100 }
    };
    if args.densities {
        for d in ["compact", "normal", "relaxed"] {
            ui.density = d.into();
            println!("\x1b[1m{d}\x1b[0m");
            println!("{}", popup::preview(&ui, cols));
        }
        return;
    }
    let names: Vec<String> =
        if args.all { theme::NAMES.iter().map(|s| s.to_string()).collect() } else { vec![ui.theme.clone()] };
    for name in names {
        ui.theme = name.clone();
        println!("\x1b[1m{name}\x1b[0m  (border = {}, density = {}, icons = {})", ui.border, ui.density, ui.icons);
        println!("{}", popup::preview(&ui, cols));
    }
}

fn default_shell() -> PathBuf {
    std::env::var_os("SHELL").map(PathBuf::from).filter(|p| Path::new(p).exists()).unwrap_or_else(|| "/bin/zsh".into())
}

/// Bytes of padding the zellij re-exec appends to `tabbify shell`'s argv.
const ARGV_PAD: usize = 4096;

/// Re-exec `tabbify shell` with a pad positional so the zellij mirror has room for long
/// command lines. Detected by the positional, not an env var, so a nested tabbify doesn't inherit it.
fn needs_reexec(args: &[std::ffi::OsString], zellij: bool, no_mirror: bool) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let padded = args.iter().skip(2).any(|a| !a.is_empty() && a.as_bytes().iter().all(|&b| b == b' '));
    cfg!(target_os = "linux")
        && zellij
        && !no_mirror
        && args.get(1).is_some_and(|a| a == "shell")
        && !padded
}

fn main() -> Result<()> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let zellij = std::env::var_os("ZELLIJ").is_some();
    if needs_reexec(&args, zellij, std::env::var_os("TABBIFY_NO_MIRROR").is_some()) {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new("/proc/self/exe")
            .arg0(&args[0])
            .args(&args[1..])
            .arg(" ".repeat(ARGV_PAD))
            .exec();
        log(&format!("mirror: re-exec with argv padding failed: {err}; continuing unpadded"));
    }
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Shell { shell, login, pad } => {
            if pad.is_some() {
                // The /proc/self/exe re-exec names the process "exe"; restore "tabbify" before spawning threads.
                // SAFETY: PR_SET_NAME reads a NUL-terminated string of at most 16 bytes.
                #[cfg(target_os = "linux")]
                unsafe {
                    libc::prctl(libc::PR_SET_NAME, c"tabbify".as_ptr())
                };
            }
            icons::detect_in_background();
            let code = session::run(shell.unwrap_or_else(default_shell), login)?;
            std::process::exit(code);
        }
        Cmd::Init { shell, part } => print!("{}", init_script(shell, part)),
        Cmd::Install { shell } => install(shell.map(Ok).unwrap_or_else(ShellKind::current)?)?,
        Cmd::Uninstall { shell } => uninstall(shell.map(Ok).unwrap_or_else(ShellKind::current)?)?,
        Cmd::Specs { cmd: SpecsCmd::List } => list_specs(),
        Cmd::Specs { cmd: SpecsCmd::Path } => {
            println!("user specs: {}", store::user_dir().display());
            println!("--help cache: {}", help::cache_dir().display());
        }
        Cmd::Specs { cmd: SpecsCmd::Generators } => {
            for (id, what) in specs::gens::BUILTIN_IDS {
                println!("{id:<30} {what}");
            }
            println!("{:<30} <prefix> then local paths (e.g. files:file://)", "files:<prefix>");
            println!("{:<30} names of a kubernetes resource type", "kube:names:<type>");
        }
        Cmd::Specs { cmd: SpecsCmd::ClearCache } => {
            let dir = help::cache_dir();
            if dir.exists() {
                std::fs::remove_dir_all(&dir)?;
            }
            println!("tabbify: cleared {}", dir.display());
        }
        Cmd::Complete { cwd, aliases, line } => complete(&line, cwd, aliases)?,
        Cmd::Doctor => doctor()?,
        Cmd::Update { check, force } => update(check, force)?,
        Cmd::Icons => list_icons(),
        Cmd::Keys => keys::show(&keys::Bindings::new(&config::Config::load().keys))?,
        Cmd::Preview(args) => preview(args),
        Cmd::Config { what: ConfigCmd::Path } => println!("{}", config::config_path().display()),
        Cmd::Config { what: ConfigCmd::Default } => {
            print!("{}", toml::to_string_pretty(&config::Config::default())?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod shell_tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<std::ffi::OsString> {
        v.iter().map(std::ffi::OsString::from).collect()
    }

    #[test]
    fn needs_reexec_only_for_shell_in_zellij() {
        let a = os(&["tabbify", "shell", "--shell", "/usr/bin/zsh"]);
        assert_eq!(needs_reexec(&a, true, false), cfg!(target_os = "linux"));
        assert!(!needs_reexec(&a, false, false));
        assert!(!needs_reexec(&a, true, true));
        assert!(!needs_reexec(&os(&["tabbify", "doctor"]), true, false));
    }

    #[test]
    fn mirror_status_off_outside_zellij() {
        assert_eq!(mirror_status(Some(1), false, false, || None), "[off ] zellij mirror: not in zellij");
    }

    #[test]
    fn mirror_status_off_when_disabled() {
        assert_eq!(mirror_status(Some(1), true, true, || None), "[off ] zellij mirror: TABBIFY_NO_MIRROR set");
    }

    #[test]
    fn mirror_status_on_when_cmdline_shows_doctor() {
        let mut area = b"tabbify\0doctor\0".to_vec();
        area.resize(4105, 0);
        assert_eq!(
            mirror_status(Some(1), true, false, || Some(area.clone())),
            "[ok  ] zellij mirror: on, argv area 4105 bytes"
        );
    }

    #[test]
    fn mirror_status_on_with_full_tabbify_path() {
        let mut area = b"/usr/bin/tabbify\0doctor\0".to_vec();
        area.resize(4105, 0);
        assert_eq!(
            mirror_status(Some(1), true, false, || Some(area.clone())),
            "[ok  ] zellij mirror: on, argv area 4105 bytes"
        );
        let other = b"/usr/bin/liggy\0doctor\0".to_vec();
        assert!(mirror_status(Some(1), true, false, || Some(other.clone())).contains("not mirroring"));
        let other = b"tabbify\0doctors\0".to_vec();
        assert!(mirror_status(Some(1), true, false, || Some(other.clone())).contains("not mirroring"));
    }

    #[test]
    fn mirror_status_not_mirroring() {
        let orig = b"tabbify\0shell\0--shell\0/usr/bin/zsh\0".to_vec();
        let want = "[off ] zellij mirror: not mirroring (see TABBIFY_LOG)";
        assert_eq!(mirror_status(Some(1), true, false, || Some(orig.clone())), want);
        assert_eq!(mirror_status(Some(1), true, false, || None), want);
    }

    #[test]
    fn mirror_status_without_tabbify_pid_is_not_a_tabbify_pane() {
        let want = "[off ] zellij mirror: not in a tabbify pane";
        assert_eq!(mirror_status(None, true, false, || panic!("read without TABBIFY_PID")), want);
    }

    #[test]
    fn shell_pad_accepts_only_spaces() {
        let pad = " ".repeat(4096);
        assert!(Cli::try_parse_from(["tabbify", "shell", "--shell", "/usr/bin/zsh", pad.as_str()]).is_ok());
        assert!(Cli::try_parse_from(["tabbify", "shell"]).is_ok());
        assert!(Cli::try_parse_from(["tabbify", "shell", "typo"]).is_err());
        assert!(Cli::try_parse_from(["tabbify", "shell", " x "]).is_err());
        assert!(Cli::try_parse_from(["tabbify", "shell", ""]).is_err());
    }

    #[test]
    fn needs_reexec_false_when_padded() {
        let pad = " ".repeat(4096);
        let a = os(&["tabbify", "shell", "--shell", "/usr/bin/zsh", &pad]);
        assert!(!needs_reexec(&a, true, false));
    }

    #[test]
    fn tabbify_spec_matches_the_cli() {
        use clap::CommandFactory;
        fn check(cli: &clap::Command, spec: &spec::Subcommand, path: &str) {
            for a in cli.get_arguments() {
                if let Some(long) = a.get_long() {
                    let flag = format!("--{long}");
                    assert!(
                        spec.options.iter().any(|o| o.meta.names.contains(&flag)),
                        "specs/tabbify/tabbify.json lacks `{path} {flag}`"
                    );
                }
            }
            for sub in cli.get_subcommands() {
                let name = sub.get_name();
                let s =
                    spec.find_subcommand(name).unwrap_or_else(|| panic!("specs/tabbify/tabbify.json lacks `{path} {name}`"));
                // clap's `help` repeats every command under it; nothing to complete there.
                if name != "help" {
                    check(sub, s, &format!("{path} {name}"));
                }
            }
        }
        let mut cli = Cli::command();
        cli.build();
        check(&cli, &specs::builtin("tabbify").expect("specs/tabbify/tabbify.json"), "tabbify");
    }

}
