# tabbify: wrap this interactive bash in the tabbify PTY (once per terminal).
if [[ $- == *i* && -z "$TABBIFY_DISABLE" && -t 0 && -t 1 && "$TERM" != dumb && -z "$INSIDE_EMACS" \
      && -x "@TABBIFY_BIN@" && "$TABBIFY_TTY" != "$(tty)" ]]; then
  if shopt -q login_shell; then
    exec "@TABBIFY_BIN@" shell --shell "$BASH" --login
  else
    exec "@TABBIFY_BIN@" shell --shell "$BASH"
  fi
fi
