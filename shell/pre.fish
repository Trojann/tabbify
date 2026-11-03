# Wrap fish in tabbify unless tabbify itself started it; `fish` typed in another shell
# inside tabbify gets its own tabbify, like a tmux pane.
if status is-interactive; and test -z "$TABBIFY_DISABLE"; and isatty stdin; and isatty stdout; and test "$TERM" != dumb; and test -z "$INSIDE_EMACS"; and test -x @TABBIFY_BIN@
    # Parent pid: /proc on Linux, `ps` elsewhere (macOS).
    set -l __tabbify_parent
    if test -r /proc/$fish_pid/stat
        set __tabbify_parent (string split ' ' -- (string replace -r '^.*\) ' '' < /proc/$fish_pid/stat))[2]
    else
        set __tabbify_parent (string trim -- (command ps -o ppid= -p $fish_pid 2>/dev/null))
    end
    if test "$TABBIFY_TTY" != (tty); or test "$__tabbify_parent" != "$TABBIFY_PID"
        if status is-login
            exec @TABBIFY_BIN@ shell --shell (status fish-path) --login
        else
            exec @TABBIFY_BIN@ shell --shell (status fish-path)
        end
    end
end
