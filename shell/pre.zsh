# tabbify: wrap this interactive shell in the tabbify PTY (once per terminal).
if [[ -o interactive && -z "$TABBIFY_DISABLE" && "$TABBIFY_TTY" != "$TTY" && -t 0 && -t 1 \
      && "$TERM" != dumb && -z "$INSIDE_EMACS" && -x "@TABBIFY_BIN@" ]]; then
  # This shell's binary: /proc on Linux, `ps` elsewhere (macOS).
  __tabbify_zsh=${${:-/proc/$$/exe}:A}
  if [[ ! -x $__tabbify_zsh ]]; then
    __tabbify_zsh=$(builtin command ps -p $$ -o comm= 2>/dev/null)
    __tabbify_zsh=${__tabbify_zsh#-}
    [[ $__tabbify_zsh == /* ]] || __tabbify_zsh=${commands[${__tabbify_zsh:-zsh}]:-${SHELL:-/bin/zsh}}
  fi
  if [[ -o login ]]; then
    builtin exec "@TABBIFY_BIN@" shell --shell "$__tabbify_zsh" --login
  else
    builtin exec "@TABBIFY_BIN@" shell --shell "$__tabbify_zsh"
  fi
  unset __tabbify_zsh
fi
