# tabbify: bash has no "line changed" hook, so tabbify reads the line off the screen
# between the prompt marker and the cursor.
if [[ $- == *i* && -z "$TABBIFY_DISABLE" && -n "$TABBIFY_TTY" && "$TABBIFY_TTY" == "$(tty)" && -z "$__tabbify_loaded" ]]; then
  __tabbify_loaded=1
  __tabbify_aliases=
  __tabbify_once=

  __tabbify_prompt() {
    local status=$?
    printf '\e]6973;C;%s\a\e]6973;P;%s\a' "${PWD//[$'\a\e']/}" "${PATH//[$'\a\e']/}" >/dev/tty
    local k a=
    for k in "${!BASH_ALIASES[@]}"; do a+="$k=${BASH_ALIASES[$k]}"$'\x1e'; done
    a=${a%$'\x1e'}
    a=${a//[$'\a\e']/}
    if [[ $a != "$__tabbify_aliases" ]]; then
      __tabbify_aliases=$a
      printf '\e]6973;A;%s\a' "$a" >/dev/tty
    fi
    if [[ -z $__tabbify_once ]]; then
      __tabbify_once=1
      local -a b f
      mapfile -t b < <(compgen -b)
      mapfile -t f < <(compgen -A function -X '_*')
      local IFS=$'\x1e'
      printf '\e]6973;K;%s\a\e]6973;N;%s\a' "${b[*]}" "${f[*]}" >/dev/tty
    fi
    # Re-append the marker when another tool (starship, …) rebuilds PS1.
    if [[ $PS1 != *'6973;S'* ]]; then
      # A prompt ending in an odd backslash would swallow ours: complete it.
      local t=${PS1##*[!\\]}
      (( ${#t} % 2 )) && PS1+='\'
      PS1+='\[\e]6973;S\a\]'
    fi
    return $status
  }
  if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND+=(__tabbify_prompt)
  else
    PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND$'\n'}__tabbify_prompt"
  fi
  # Prepended: vte.sh (GNOME Terminal) ends PS0 with `ESC \`, which would swallow our `\e`.
  [[ $PS0 == *'6973;F'* ]] || PS0='\e]6973;F\a'"${PS0:-}"

  # tabbify writes `<delete>;<back>;<text>` to $TABBIFY_INSERT_FILE, then sends ESC [9147~.
  __tabbify_insert() {
    local p
    p=$(<"$TABBIFY_INSERT_FILE") || return
    local del=${p%%;*}
    p=${p#*;}
    local back=${p%%;*}
    local text=${p#*;}
    local left=${READLINE_LINE:0:READLINE_POINT} right=${READLINE_LINE:READLINE_POINT}
    if (( del >= ${#left} )); then
      left=
    elif (( del > 0 )); then
      left=${left:0:${#left}-del}
    fi
    READLINE_LINE=$left$text$right
    READLINE_POINT=$(( ${#left} + ${#text} - back ))
  }
  bind -x '"\e[9147~": __tabbify_insert'
fi
