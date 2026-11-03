# tabbify: report the ZLE edit buffer to the tabbify PTY and accept insertions from it.
if [[ -o interactive && -z "$TABBIFY_DISABLE" && -n "$TABBIFY_TTY" && "$TABBIFY_TTY" == "$TTY" ]] && (( ! ${+_tabbify_loaded} )); then
  typeset -g _tabbify_loaded=1 _tabbify_last=

  _tabbify_report() {
    builtin emulate -L zsh
    local msg
    # No popup during history browsing or zsh's own completion: those keys stay with zsh.
    if (( ${ISEARCHMATCH_ACTIVE:-0} )) || [[ $KEYMAP == (vicmd|menuselect) ]] || [[ -n $PREBUFFER ]] \
        || [[ $LASTWIDGET == (*history*|*beginning-search*|up-line*|down-line*|*-search-*|*complete*|*expand*|menu-*) ]]; then
      msg=$'\e]6973;H\a'
    else
      # Mask control chars so they can't end the OSC; replacing keeps $CURSOR valid.
      msg=$'\e]6973;B;'"${CURSOR};${BUFFER//[$'\a\e']/ }"$'\a'
    fi
    [[ $msg == $_tabbify_last ]] && return
    _tabbify_last=$msg
    builtin print -rn -- "$msg" >$TTY
  }
  _tabbify_line_init() { _tabbify_last=; _tabbify_report }
  _tabbify_line_finish() {
    _tabbify_last=
    builtin print -rn -- $'\e]6973;F\a' >$TTY
  }

  # tabbify sends: ESC [9147~ <delete>;<back>;<text> BEL
  _tabbify_insert() {
    builtin emulate -L zsh
    local c payload=
    while builtin read -k 1 c; do
      [[ $c == $'\a' ]] && break
      payload+=$c
    done
    local del=${payload%%;*}
    payload=${payload#*;}
    local back=${payload%%;*}
    local text=${payload#*;}
    (( del > 0 )) && LBUFFER=${LBUFFER[1,-del-1]}
    LBUFFER+=$text
    (( back > 0 )) && (( CURSOR -= back ))
    _tabbify_last=
    # zsh-autosuggestions: drop the stale suggestion and fetch a fresh one.
    unset POSTDISPLAY
    (( ${+functions[_zsh_autosuggest_fetch]} )) && _zsh_autosuggest_fetch
  }
  # zsh-autosuggestions' widget wrapper would restore the old suggestion mid-read.
  typeset -ga ZSH_AUTOSUGGEST_IGNORE_WIDGETS
  ZSH_AUTOSUGGEST_IGNORE_WIDGETS+=(_tabbify_insert)

  # Generators need the shell's cwd and PATH; aliases/functions sent on change (split by \x1e).
  typeset -g _tabbify_aliases= _tabbify_functions= _tabbify_builtins_sent=
  _tabbify_precmd() {
    builtin emulate -L zsh
    builtin print -rn -- $'\e]6973;C;'"${PWD//[$'\a\e']/}"$'\a'$'\e]6973;P;'"${PATH//[$'\a\e']/}"$'\a' >$TTY
    local k
    local -a pairs
    for k in ${(k)aliases}; do pairs+=("$k=${aliases[$k]}"); done
    local a=${(pj:\x1e:)pairs}
    a=${a//[$'\a\e']/}
    if [[ $a != $_tabbify_aliases ]]; then
      _tabbify_aliases=$a
      builtin print -rn -- $'\e]6973;A;'"$a"$'\a' >$TTY
    fi
    if [[ -z $_tabbify_builtins_sent ]]; then
      _tabbify_builtins_sent=1
      builtin print -rn -- $'\e]6973;K;'"${(pj:\x1e:)${(@k)builtins:#[^a-z]*}}"$'\a' >$TTY
    fi
    local f=${(pj:\x1e:)${(k)functions:#[_+.-]*}}
    if [[ $f != $_tabbify_functions ]]; then
      _tabbify_functions=$f
      builtin print -rn -- $'\e]6973;N;'"${f//[$'\a\e']/}"$'\a' >$TTY
    fi
  }
  builtin autoload -Uz add-zsh-hook
  add-zsh-hook precmd _tabbify_precmd

  builtin autoload -Uz add-zle-hook-widget
  add-zle-hook-widget line-init _tabbify_line_init
  add-zle-hook-widget line-pre-redraw _tabbify_report
  add-zle-hook-widget line-finish _tabbify_line_finish
  builtin zle -N _tabbify_insert
  builtin bindkey -M emacs '\e[9147~' _tabbify_insert
  builtin bindkey -M viins '\e[9147~' _tabbify_insert
  builtin bindkey -M vicmd '\e[9147~' _tabbify_insert
fi
