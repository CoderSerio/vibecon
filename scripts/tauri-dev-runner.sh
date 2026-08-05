#!/bin/zsh

set -euo pipefail

# Tauri normally delegates `dev` to `cargo run`. On macOS that produces a
# linker-signed binary whose designated requirement is only its changing
# cdhash, so Accessibility permission is lost after every Rust rebuild.
# Build first, sign with a stable local development identity, then execute the
# same binary. Non-run cargo commands are passed through untouched.
if [[ "${1:-}" != "run" ]]; then
  exec cargo "$@"
fi

shift
build_args=(build)
app_args=()
profile=debug
target_triple=""
after_separator=false

while (( $# > 0 )); do
  argument="$1"
  shift
  if [[ "$after_separator" == true ]]; then
    app_args+=("$argument")
    continue
  fi
  if [[ "$argument" == "--" ]]; then
    after_separator=true
    continue
  fi
  build_args+=("$argument")
  if [[ "$argument" == "--release" ]]; then
    profile=release
  elif [[ "$argument" == "--target" && $# > 0 ]]; then
    target_triple="$1"
  fi
done

cargo "${build_args[@]}"

target_root="${CARGO_TARGET_DIR:-target}"
if [[ -n "$target_triple" ]]; then
  executable="$target_root/$target_triple/$profile/vibecon"
else
  executable="$target_root/$profile/vibecon"
fi

if [[ ! -x "$executable" ]]; then
  print -u2 "VibeCon dev runner could not find $executable"
  exit 1
fi

project_root="$(cd "$(dirname "$0")/.." && pwd)"
app_bundle="$target_root/$profile/VibeCon Dev.app"
app_binary="$app_bundle/Contents/MacOS/vibecon"
app_resources="$app_bundle/Contents/Resources"
mkdir -p "$(dirname "$app_binary")" "$app_resources"
app_bundle="$(cd "$(dirname "$app_bundle")" && pwd)/$(basename "$app_bundle")"
app_binary="$app_bundle/Contents/MacOS/vibecon"
app_resources="$app_bundle/Contents/Resources"
cp "$executable" "$app_binary"
cp "$project_root/src-tauri/Info.dev.plist" "$app_bundle/Contents/Info.plist"
app_version="$(plutil -extract version raw "$project_root/src-tauri/tauri.conf.json")"
plutil -replace CFBundleShortVersionString -string "$app_version" "$app_bundle/Contents/Info.plist"
if [[ -f "$project_root/src-tauri/icons/icon.icns" ]]; then
  cp "$project_root/src-tauri/icons/icon.icns" "$app_resources/icon.icns"
fi

identity="${VIBECON_CODESIGN_IDENTITY:-}"
if [[ -z "$identity" ]]; then
  identity="$(security find-identity -v -p codesigning 2>/dev/null \
    | sed -n 's/.*"\(Apple Development:[^"]*\)".*/\1/p' \
    | head -n 1)"
fi

if [[ -n "$identity" ]]; then
  codesign \
    --force \
    --deep \
    --sign "$identity" \
    --identifier io.coderserio.vibecon.dev \
    --timestamp=none \
    "$app_bundle"
  print "VibeCon dev runner: signed VibeCon Dev.app with $identity"
else
  codesign --force --deep --sign - --identifier io.coderserio.vibecon.dev "$app_bundle"
  print -u2 "VibeCon dev runner: no Apple Development identity found; VibeCon Dev.app uses an unstable ad-hoc signature"
fi

# Launch through Launch Services so macOS associates the process with the
# VibeCon Dev.app identity that appears in Accessibility settings. Executing
# Contents/MacOS/vibecon directly makes TCC treat it as a command-line process.
# Tauri can terminate the previous runner without giving its shell trap time
# to run, so remove only processes whose command resolves to this exact bundle
# before launching the replacement.
for existing_pid in $(pgrep -f "$app_binary" 2>/dev/null || true); do
  existing_command="$(ps -p "$existing_pid" -o command= 2>/dev/null || true)"
  if [[ "$existing_command" == "$app_binary"* ]]; then
    kill -TERM "$existing_pid"
  fi
done
for _attempt in {1..20}; do
  if ! pgrep -f "$app_binary" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
open -n "$app_bundle" --args "${app_args[@]}"
app_pid=""

for _attempt in {1..50}; do
  candidate_pid="$(pgrep -n -f "$app_binary" 2>/dev/null || true)"
  if [[ -n "$candidate_pid" ]]; then
    candidate_command="$(ps -p "$candidate_pid" -o command= 2>/dev/null || true)"
    if [[ "$candidate_command" == *"$app_binary"* ]]; then
      app_pid="$candidate_pid"
      break
    fi
  fi
  sleep 0.1
done

cleanup_dev_app() {
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill -TERM "$app_pid"
  fi
}

trap cleanup_dev_app INT TERM EXIT
if [[ -z "$app_pid" ]]; then
  print -u2 "VibeCon dev runner could not identify the launched VibeCon Dev.app process"
  exit 1
fi
while kill -0 "$app_pid" 2>/dev/null; do
  sleep 0.25
done
trap - INT TERM EXIT
