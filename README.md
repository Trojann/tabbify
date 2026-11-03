<p align="center">
  <img src="assets/logo.svg" width="120" alt="Tabbify logo">
</p>

<h1 align="center">Tabbify</h1>

<p align="center">
  A suggestion popup for your shell prompt.<br>
  Single Rust binary · zsh, bash, fish · Linux and macOS · any terminal emulator
</p>

<p align="center">
  <a href="https://github.com/Trojann/tabbify/releases/latest"><img src="https://img.shields.io/github/v/release/Trojann/tabbify" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/Trojann/tabbify" alt="MIT license"></a>
</p>

```
❯ git ch
   ╭──────────────────────────────────────────────────────────╮
   │  ▸ checkout       Switch branches or restore working tree │
   │  ▸ cherry-pick    Apply the changes introduced by some …  │
   ╰──────────────────────────────────────────────────────────╯
```

## Quick start

```bash
curl -fsSL https://github.com/Trojann/tabbify/releases/latest/download/install.sh | sh
tabbify install        # hooks into your shell's rc file
exec $SHELL        # or open a new tab
tabbify doctor         # confirms everything is wired up
```

Type `git ` at your prompt and a popup opens under the cursor.

## Why use it

Normal `Tab` completion prints a bare list of words, and it only knows what each tool's
completion script exposes. Tabbify watches the line while you type and shows a popup
with context, closer to what a code editor offers:

- a one-line description next to every subcommand and flag;
- values read from your machine: git branches, running containers, systemd units, ssh hosts,
  make targets, npm scripts, zoxide directories, process IDs;
- live package search for `npm i`, `cargo add`, `docker pull`, `pip install`, `snap install`;
- your own aliases, functions and builtins (`gco ` behaves like `git checkout `).

How it knows about commands, in short:

- about 740 commands ship inside the binary (a conversion of the
  [withfig/autocomplete](https://github.com/withfig/autocomplete) specs, plus hand-written ones);
- any other program is learned from its `--help` output or man page;
- no daemon, no account, no telemetry; the only data that leaves your machine is the partial
  package name in package searches (switchable off).

It works in tmux and zellij, and the popup is drawn by the program itself, so no terminal-specific
feature is needed.

## Installing

Requirements: zsh 5.3+, bash 4.4+ or fish 4.2.1+. macOS ships bash 3.2, so use zsh (the default) or
install a newer bash or fish. `curl` is optional (package search); a
[Nerd Font](https://www.nerdfonts.com) is optional (icons).

### Release binary

`install.sh` picks the right archive for your OS and CPU, verifies its sha256 and drops `tabbify` into
`~/.local/bin`.

| Variable | Effect |
| --- | --- |
| `TABBIFY_INSTALL_DIR` | install somewhere else |
| `TABBIFY_VERSION` | install a specific version, e.g. `TABBIFY_VERSION=1.0.6` |

Archives published with each [release](https://github.com/Trojann/tabbify/releases/latest), each with a `.sha256`:

| File | Platform |
| --- | --- |
| `tabbify-<version>-x86_64-unknown-linux-musl.tar.gz` | Linux x86_64, static |
| `tabbify-<version>-aarch64-unknown-linux-musl.tar.gz` | Linux ARM64, static |
| `tabbify-<version>-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64, glibc 2.35+ |
| `tabbify-<version>-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64, glibc 2.35+ |
| `tabbify-<version>-aarch64-apple-darwin.tar.gz` | macOS, Apple silicon |
| `tabbify-<version>-x86_64-apple-darwin.tar.gz` | macOS, Intel |

### Build from source

Needs [Rust](https://rustup.rs) 1.85+.

```bash
git clone https://github.com/Trojann/tabbify.git
cd tabbify
cargo install --path .
```

### Nix and home-manager

The flake exposes a home-manager module that installs the program, writes its config and enables
the shell integrations.

```nix
# flake.nix
inputs.tabbify = {
  url = "github:Trojann/tabbify";
  inputs.nixpkgs.follows = "nixpkgs";
};

# home.nix
{ inputs, ... }:
{
  imports = [ inputs.tabbify.homeModules.default ];

  programs.tabbify = {
    enable = true;
    settings = {            # rendered to config.toml
      ui.theme = "catppuccin";
      behavior.online = false;
    };
  };
}
```

Disable a single shell with `programs.tabbify.enableZshIntegration = false;` (likewise
`enableBashIntegration`, `enableFishIntegration`). Under Nix, update by bumping the flake input,
not with `tabbify update`.

### Shell integration

`tabbify install [zsh|bash|fish]` (default: `$SHELL`) inserts one block at the top and one at the
bottom of your rc file, after saving a copy as `<rc>.tabbify-backup`. Manual setup:

```bash
# zsh: ~/.zshrc, bash: ~/.bashrc
eval "$(tabbify init zsh pre)"     # very first line   (bash: tabbify init bash pre)
eval "$(tabbify init zsh post)"    # very last line    (bash: tabbify init bash post)
```

```fish
# ~/.config/fish/config.fish
eval (tabbify init fish pre | string collect)    # first line
eval (tabbify init fish post | string collect)   # last line
```

### Updating and removing

- `tabbify update` downloads the newest release, checks its sha256 and replaces the binary;
  `tabbify update --check` only reports. Already-open tabs keep the old version until reopened.
- `TABBIFY_DISABLE=1 zsh` starts one shell without it.
- `tabbify uninstall [shell]` removes the blocks from your rc file.

## Everyday use

| Key | What it does |
| --- | --- |
| `Tab`, `↓` | next item |
| `Shift-Tab`, `↑` | previous item (from the first one, closes the popup) |
| `Enter` | take the highlighted item |
| `Esc` | close the popup (press twice, then `Enter` runs the line as typed) |
| `Ctrl+Space` | open or close the popup manually |

Behaviors worth knowing:

- Command names don't trigger the popup on their own, so `ls` + `Enter` just runs. Use
  `Ctrl+Space` to list commands, or set `complete_commands = true`.
- `Enter` finishes the current word (`go bui` becomes `go build`); after a space it takes the first
  item (`git checkout ` becomes `git checkout main`); on an already complete word it executes
  the line. Two `Enter` presses mean "take it, then run".
- No trailing space is inserted after a pick, flags included. Type it to see the next level.
- Editors and viewers (`code`, `vim`, `nvim`, `cat`, `less`, `xdg-open`, …) list files before
  options; everything else lists options first. Extend with `files_first_commands`.
- Folder pickers (`cd`, `mkdir`, …) stay open inside the chosen folder. The first rows are
  **↵ Enter** (run now) and **Back** (parent folder). Path flags such as `cargo install --path`
  also offer `.`.
- Shell history, the shell's own `Tab`, and fullscreen helpers such as fzf's `Ctrl+T` are left
  alone: the popup hides while they are active and returns when you type.

## Configuration

The file is `~/.config/tabbify/config.toml` on Linux and `~/Library/Application Support/tabbify/config.toml`
on macOS. `tabbify config path` prints it and `tabbify config default` prints every option with defaults.

```toml
[ui]
theme = "default"          # catppuccin, nord, dracula, gruvbox, tokyonight, light, terminal
border = "rounded"         # square, none
density = "normal"         # compact, relaxed
icons = "auto"             # nerd, unicode, none
width = "auto"             # a column count (60) or a share such as "50%"
height = 8                 # rows, or "30%"
description_position = "right"   # "bottom" puts descriptions in a strip below the list

[colors]                   # per-color overrides: "#rrggbb", 0-255, or "default"
# bg = "#1e1e2e"

[icons]                    # override icons by name (`tabbify icons` lists them)
# folder = "📁"                                    # glyph only
# git-branch = { glyph = "", color = "#f39c12" }  # glyph + color
# option = { color = "214" }                       # color only
# "*.rs" = "🦀"                                    # by extension
# "Dockerfile" = "🐳"                              # by file name

[keys]                     # `tabbify keys` shows the name of any key you press
accept = ["enter"]
next = ["tab", "down", "ctrl-n"]
prev = ["shift-tab", "up", "ctrl-p"]
dismiss = ["esc"]
toggle = ["ctrl-space"]

[behavior]
enter_accepts = "auto"     # "after-space", "always", "never"
insert_space = false       # append a space after every pick
complete_commands = false  # popup for command names as well
online = true              # query package registries while typing
disabled_commands = []     # no popup for these commands
files_first_commands = ["code", "nvim", "…"]
```

Key names combine modifiers with a key: `ctrl-n`, `alt-j`, `shift-up`, `ctrl-alt-x`, `f5`,
`pagedown`. Anything else can be given as a raw escape sequence (`"\\x1b[1;5C"`). Keys your terminal
swallows (commonly Ctrl+Tab) never arrive.

Icon rules go from broad to narrow: `file = { color = "1" }` tints all files, then `"*.rs"` changes
only Rust files. Unknown names produce a warning. With `icons = "auto"`, Nerd Fonts are detected via
`fc-list`, or by font file names in `~/Library/Fonts` on macOS; force them with `icons = "nerd"`.

Preview styling without editing anything:

```bash
tabbify preview --all
tabbify preview --theme nord --density compact --border square
tabbify preview --description bottom --width 60
tabbify icons
```

Changes take effect in new tabs.

## How suggestions are resolved

For each command, sources are consulted in this order:

1. your own specs: `<config dir>/specs/<command>.json` or `.toml`;
2. hand-written specs in [`specs/tabbify/`](specs/tabbify), merged with the converted spec of the same
   command when one exists;
3. converted specs in [`specs/fig/`](specs/fig);
4. the command's `--help` or man page, parsed and cached per executable version. `--help` is only
   run for real binaries and Python/Node/Ruby scripts, with no stdin, no display and a 2 s limit;
   other files get their man page read. `help_parsing = false` turns this off.

`tabbify specs list` reveals which source a command uses.

Online search (`npm i`, `pnpm add`, `cargo add`, `docker pull`, `pip install`, `snap install`)
sends the partial package name to the registry via `curl` and caches replies for 30 minutes.
`online = false` disables it.

### Writing your own spec

A spec lists a command's subcommands, options and arguments, using the
[Fig spec vocabulary](https://fig.io/docs/reference) (static parts only). Example,
`<config dir>/specs/deploy.toml`:

```toml
name = "deploy"
description = "Deploy the app"

[[options]]
name = ["-e", "--env"]
description = "Target environment"
args = { name = "env", suggestions = ["staging", "production"] }

[[subcommands]]
name = "logs"
description = "Tail the logs of a service"
args = { name = "service", generators = { script = ["deploy", "list-services"] } }
```

An argument may carry fixed `suggestions`, a `template` of `"filepaths"` or `"folders"`, a script
generator whose output lines become entries, or a native one such as
`generators = [{ builtin = "git:branches" }]` (`tabbify specs generators` lists them).
Try it: `tabbify complete "deploy -e "`.

## Internals

`tabbify shell` starts your shell inside a pseudo-terminal and relays all I/O. A virtual screen
tracks what's displayed, so the popup can be painted below the cursor and the covered cells
restored when it closes. That design is why it works in every terminal and under Wayland, where
applications cannot place floating windows.

The shell hooks report the current line: zsh on each redraw, bash through a marker in the prompt
that lets the line be read off the screen, fish on request after every key. To insert a
suggestion, a private key sequence triggers a shell function that edits the buffer. Slow lookups
(subprocesses, network) happen on worker threads so typing never stalls.

## zellij (Linux)

Inside zellij the pane's process is `tabbify shell`, so a resurrected session would reopen every pane
as `tabbify shell` in the starting folder. To prevent that, once per second the program copies the
foreground application's command line and working directory onto itself. zellij then records
`nvim foo`, `htop`, or a plain shell, in the right folder, and restores accordingly.

```bash
tabbify doctor                  # prints "[ok  ] zellij mirror: on" in a zellij pane
TABBIFY_NO_MIRROR=1 zellij      # disable the mirror, completion still works
```

Outside zellij nothing changes. Shells, wrappers (`env`, `nohup`, `timeout`, …) and risky-to-rerun
commands (`sudo`, `rm`, `mv`, `dd`, `kill`, `git`, …) are stored as a plain shell. This relies on
`/proc`, so it is unavailable on macOS.

**Do not resurrect with `zellij attach --force-run-commands`**: the per-pane ENTER prompt is
your safety net.

Caveats:

- `pkill -f <app>` inside zellij also matches the pane and closes it; `pkill <app>`,
  `killall <app>` and `pgrep -x <app>` don't, as the process name remains `tabbify`.
- A pipeline is saved as its first command (`cat log | less` becomes `cat log`).
- A terminal launched from a zellij pane inherits `$ZELLIJ` and runs the mirror too; this only
  affects how `ps` displays it.
- Restored apps get the zellij server's `PATH`.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| No popup | `tabbify doctor` must say "running inside tabbify"; open a new tab after installing or updating |
| Odd or missing suggestions | `tabbify complete "<line>"` prints what would be offered |
| A key does nothing | `tabbify keys`, then press the key: it shows whether it arrives and its name |
| Stale suggestions for a tool | `tabbify specs clear-cache` forces `--help` to be parsed again |
| Anything else | run `TABBIFY_LOG=1 zsh` (or bash/fish) and read `tabbify.log` (`~/.local/state/tabbify/` on Linux, the temp folder on macOS) |

## Known limits

- bash: the line comes from the screen, so a cursor in a wrapped line at the bottom of a full
  screen can briefly confuse it until the next key.
- fish: `SIGUSR1` is used for line reports and Ctrl+Alt+Shift+F12 for insertion; fish's own
  `complete` definitions and alias bodies are not imported; Starship is untested with fish.
- Specs derived from `--help` have no live values (branches and the like).
- Roughly a quarter of the dynamic generators in the Fig specs relied on JavaScript and could not
  be converted; their static options and subcommands are kept.
- macOS: no zellij mirror, prebuilt-only `tabbify update`, bash 3.2 unsupported.

## Hacking on it

```bash
cargo test                      # unit tests, including every spec in specs/tabbify/
cargo build --release
python3 tests/fish_pty.py       # PTY suites (Linux): fish, bash/zsh, tmux, trial launcher
python3 tests/shell_smoke.py
python3 tests/tmux_placement.py
python3 tests/trial_launcher.py
python3 tests/zellij_mirror.py  # needs zsh, not zellij
tools/update-fig-specs.sh       # re-import specs/fig/ from withfig/autocomplete
```

The PTY suites need Python 3 and the shells under test, and exercise `target/release/tabbify`
(`TABBIFY_TEST_BIN` selects another binary).

```
src/                  program (session.rs: PTY + popup, engine.rs: completion,
                      popup.rs: rendering, help.rs: --help parser, specs/gens.rs: native generators,
                      procfs.rs: /proc and libproc process inspection)
shell/                zsh, bash, fish hooks
specs/tabbify/            hand-written specs (JSON)
specs/fig/            converted withfig/autocomplete specs (generated)
build.rs              compresses and embeds both spec folders
tests/                PTY tests (Python)
tools/figconv/        Fig → JSON converter (dev only)
flake.nix, nix/       flake and home-manager module
install.sh            installer attached to each release
.github/workflows/    CI and release builds
```

Adding a built-in spec: drop `specs/tabbify/<command>.json` and rebuild. Adding a new live value
source: write a generator in `src/specs/gens.rs` and register it in `builtin()`.

## Origin

Tabbify is a port of [KevenGoncalves/lig](https://github.com/KevenGoncalves/lig) (v1.0.7). This
repository adds macOS support (libproc process inspection, Nerd Font detection, macOS release
builds), renames the project and command to `tabbify`, and splits the installer and updater out
of `main.rs`. All credit for the original design and code goes to its author and contributors.

## License

MIT, see [`LICENSE`](LICENSE) (it carries the copyright notices). The specs under `specs/fig/` come
from [withfig/autocomplete](https://github.com/withfig/autocomplete), © Hercules Labs Inc. and
contributors, MIT, see [`specs/fig/LICENSE`](specs/fig/LICENSE).
