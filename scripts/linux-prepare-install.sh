#!/bin/sh
# Drop Linux app-menu entries that launch a k-agent dev binary.
# A dev binary loads http://localhost:1420 and reports that the frontend
# server is not running once `pnpm tauri dev` has stopped.
# Run this before installing a release build. Linux only.

set -eu

if [ "$(uname -s)" != "Linux" ]; then
  echo "linux-prepare-install: Linux only" >&2
  exit 1
fi

apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
removed=0
warned=""

is_dev_binary() {
  bin=$1
  case "$bin" in
    *"/target/debug/"*)
      return 0
      ;;
  esac
  return 1
}

resolve_exec() {
  raw=$1
  raw=${raw#Exec=}
  # shellcheck disable=SC2086
  set -- $raw
  cmd=${1:-}
  cmd=${cmd#\"}
  cmd=${cmd%\"}
  if [ -z "$cmd" ]; then
    return 1
  fi
  case "$cmd" in
    env)
      shift
      while [ $# -gt 0 ]; do
        case "$1" in
          *=*) shift ;;
          *)
            cmd=$1
            break
            ;;
        esac
      done
      ;;
  esac
  if [ -x "$cmd" ]; then
    printf '%s\n' "$cmd"
    return 0
  fi
  command -v "$cmd" 2>/dev/null || return 1
}

warn_unstripped() {
  bin=$1
  [ -f "$bin" ] || return 0
  if ! command -v file >/dev/null 2>&1; then
    return 0
  fi
  if file -b "$bin" 2>/dev/null | grep -q 'not stripped'; then
    echo "warning: $bin looks like a debug build (not stripped)." >&2
    echo "A debug binary expects the Vite server on port 1420. Install a release bundle instead." >&2
  fi
}

if [ -d "$apps" ]; then
  for file in "$apps"/*.desktop; do
    [ -f "$file" ] || continue
    base=$(basename "$file")
    case "$base" in
      *k-agent* | *kagent.app*) ;;
      *) continue ;;
    esac
    exec_line=$(grep -m1 -E '^Exec=' "$file" || true)
    [ -n "$exec_line" ] || continue
    bin=$(resolve_exec "$exec_line" || true)
    if is_dev_binary "$bin"; then
      rm -f "$file"
      echo "removed $file"
      removed=$((removed + 1))
      continue
    fi
    if [ -n "$bin" ]; then
      case " $warned " in
        *" $bin "*) ;;
        *)
          warn_unstripped "$bin"
          warned="$warned $bin"
          ;;
      esac
    fi
  done
fi

if command -v update-desktop-database >/dev/null 2>&1 && [ -d "$apps" ]; then
  update-desktop-database "$apps" || true
fi

echo "linux-prepare-install: removed $removed desktop entries"
echo "Install the release bundle next. Do not copy src-tauri/target/debug/k-agent onto the menu launcher."
