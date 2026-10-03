#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
for task_tool in cargo dbus-run-session gdbus gnome-keyring-daemon; do
  command -v "$task_tool" >/dev/null || { echo "Missing $task_tool" >&2; exit 1; }
done
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
task_fixture_home="$(mktemp -d)"
trap 'rm -rf "$task_fixture_home"' EXIT

# A new bus and HOME prevent all access to the user's desktop credentials.
dbus-run-session -- bash -c '
  set -euo pipefail
  export HOME="$1" XDG_DATA_HOME="$1/.local/share" XDG_CONFIG_HOME="$1/.config" XDG_RUNTIME_DIR="$1/runtime"
  mkdir -p "$XDG_DATA_HOME/keyrings"
  mkdir -m 700 "$XDG_RUNTIME_DIR"
  for collection in login other; do
    printf "[keyring]\ndisplay-name=%s\nctime=0\nmtime=0\nlock-on-idle=false\nlock-timeout=0\n" "$collection" > "$XDG_DATA_HOME/keyrings/$collection.keyring"
  done
  printf "login" > "$XDG_DATA_HOME/keyrings/default"
  gnome-keyring-daemon --foreground --components=secrets > "$HOME/keyring.log" 2>&1 &
  task_keyring_pid=$!
  trap '\''kill "$task_keyring_pid" 2>/dev/null || true; wait "$task_keyring_pid" 2>/dev/null || true'\'' EXIT
  task_ready=false
  for attempt in {1..30}; do
    if gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.freedesktop.secrets | grep -q true; then
      task_ready=true; break
    fi
    sleep 0.1
  done
  "$task_ready" || { cat "$HOME/keyring.log" >&2; exit 1; }
  ANTIGRAVITY_TEST_SECRET_SERVICE=isolated cargo test --locked \
    --manifest-path "$2/src-tauri/Cargo.toml" --lib isolated_ -- --ignored --test-threads=1
' bash "$task_fixture_home" "$task_root"
