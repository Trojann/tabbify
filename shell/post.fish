# Report the reader buffer and accept completions inside tabbify.
if status is-interactive; and test -z "$TABBIFY_DISABLE"; and test -n "$TABBIFY_TTY"; and test "$TABBIFY_TTY" = (tty); and not set -q __tabbify_loaded
    set -g __tabbify_loaded 1
    set -g __tabbify_editing 0

    function __tabbify_clean
        string replace -ar '[\a\e]' ' ' -- "$argv[1]"
    end

    function __tabbify_report --on-signal USR1
        test "$__tabbify_editing" = 1; or return
        set -l request (string collect < "$TABBIFY_QUERY_FILE")
        string match -qr '^[0-9]+$' -- "$request"; or return
        set -l msg
        set -l vi_command 0
        if contains -- "$fish_key_bindings" fish_vi_key_bindings fish_hybrid_key_bindings; and test "$fish_bind_mode" != insert
            set vi_command 1
        end
        if commandline --search-mode; or commandline --paging-mode; or test $vi_command = 1
            set msg H
        else
            set -l raw (commandline --current-buffer | string collect --allow-empty --no-trim-newlines)
            set -l buffer ''
            string match -rq '^(?<buffer>[\s\S]*)\n$' -- "$raw"
            # Replacement preserves character offsets, including newlines.
            set -l safe (__tabbify_clean "$buffer" | string collect --allow-empty --no-trim-newlines)
            string match -rq '^(?<buffer>[\s\S]*)\n$' -- "$safe"
            set msg "B;"(commandline --cursor)";$buffer"
        end
        # Acknowledge unchanged buffers too.
        printf '\e]6973;Q;%s;%s\a' "$request" "$msg" >/dev/tty
    end

    # Fish 4.2 drops the unknown CSI bash/zsh use; reserve ctrl-alt-shift-f12 (ESC [24;8~).
    function __tabbify_bind --on-variable fish_key_bindings
        for mode in (bind --list-modes)
            bind --mode $mode ctrl-alt-shift-f12 __tabbify_insert
        end
    end

    function __tabbify_prompt --on-event fish_prompt --on-event fish_cancel
        __tabbify_bind
        set -g __tabbify_editing 1
        printf '\e]6973;C;%s\a\e]6973;P;%s\a' (__tabbify_clean "$PWD") (__tabbify_clean (string join : -- $PATH)) >/dev/tty
        printf '\e]6973;N;%s\a\e]6973;K;%s\a' (string join \x1e -- (functions --names | string match -rv '^_')) (string join \x1e -- (builtin --names)) >/dev/tty
        # Ready: handlers are registered, tabbify may now send USR1.
        printf '\e]6973;R;%s\a' $fish_pid >/dev/tty
    end

    function __tabbify_finish --on-event fish_preexec
        set -g __tabbify_editing 0
        printf '\e]6973;F\a' >/dev/tty
    end

    # Payload: delete;back;prefix-length;prefix+insertion.
    function __tabbify_insert
        test -r "$TABBIFY_INSERT_FILE"; or return
        set -l payload (string collect --allow-empty --no-trim-newlines < "$TABBIFY_INSERT_FILE")
        set -l fields (string split --max 3 ';' -- "$payload")
        test (count $fields) = 4; or return
        string match -qr '^[0-9]+$' -- "$fields[1]"; or return
        string match -qr '^[0-9]+$' -- "$fields[2]"; or return
        string match -qr '^[0-9]+$' -- "$fields[3]"; or return
        set -l expected ''
        set -l text ''
        string match -rq "^(?<expected>[\\s\\S]{$fields[3]})(?<text>[\\s\\S]*)\$" -- "$fields[4]"; or return
        set -l cursor (commandline --cursor)
        set -l raw (commandline --current-buffer | string collect --allow-empty --no-trim-newlines)
        set -l buffer ''
        string match -rq '^(?<buffer>[\s\S]*)\n$' -- "$raw"
        set -l current ''
        string match -rq "^(?<current>[\\s\\S]{$cursor})" -- "$buffer"
        if test "$current" != "$expected"
            printf '\e]6973;J\a' >/dev/tty
            return
        end
        set -l keep (math "max(0, $cursor - $fields[1])")
        set -l left ''
        set -l right ''
        string match -rq "^(?<left>[\\s\\S]{$keep})" -- "$buffer"
        string match -rq "^[\\s\\S]{$cursor}(?<right>[\\s\\S]*)\$" -- "$buffer"
        set -l point (math "$keep + "(string length -- "$text")" - $fields[2]")
        test $point -ge 0; or return
        commandline --replace -- "$left$text$right"
        commandline --cursor $point
        commandline --function repaint
    end
end
