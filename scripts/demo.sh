#!/usr/bin/env bash
# Scripted demo of what Ratón Pérez adds on top of Handy (Linux/X11):
#   1. dictation cleaned up by the LLM and pasted into an editor
#   2. the writing-tools popup: preset actions, a custom instruction, Paste
#   3. presets that reshape notes: Key Points and Table
#
# A synthetic voice (Piper TTS) plays into a virtual microphone, the app is
# driven with its --toggle-* flags, and the popup with its keyboard shortcuts.
#
# Usage: scripts/demo.sh [--record out.mp4]
#
# Before running:
#   - quit Ratón Pérez (the script launches it so it opens the virtual mic)
#   - set up post-processing in the app (for example the hackathon Bedrock proxy)
#   - install: xdotool, pactl/paplay/parecord, uv; ffmpeg for --record
#
# Environment:
#   RATON_BIN    app binary (default: src-tauri/target/debug/handy)
#   DEMO_EDITOR  editor to paste into (default: xed)
#   WAIT_LLM     seconds to wait for a dictation's transcript + LLM (default: 10)
#   WAIT_ACTION  seconds to wait for each popup action (default: 6)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${RATON_BIN:-$ROOT/src-tauri/target/debug/handy}"
EDITOR_CMD="${DEMO_EDITOR:-xed}"
WAIT_LLM="${WAIT_LLM:-10}"
WAIT_ACTION="${WAIT_ACTION:-6}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/raton-perez-demo"
VOICE="en_US-lessac-medium"
SINK="raton_demo"
POPUP_TITLE="Ratón Pérez"

RECORD=""
if [[ "${1:-}" == "--record" ]]; then
  RECORD="$(realpath -m "${2:?--record needs an output path}")"
fi

need() { command -v "$1" >/dev/null || { echo "missing: $1" >&2; exit 1; }; }
for cmd in xdotool pactl paplay uvx "$EDITOR_CMD"; do need "$cmd"; done
[[ -n "$RECORD" ]] && need ffmpeg && need parecord
[[ -x "$APP" ]] || { echo "app binary not found: $APP (build it or set RATON_BIN)" >&2; exit 1; }
[[ "${XDG_SESSION_TYPE:-x11}" == "x11" ]] || { echo "needs an X11 session (xdotool)" >&2; exit 1; }
if pgrep -x "$(basename "$APP")" >/dev/null; then
  echo "Ratón Pérez is running; quit it first so it picks up the virtual mic." >&2
  exit 1
fi

# ---- Voice clips ----------------------------------------------------------

declare -A LINES=(
  [dictation]="So um, I think we should basically move the launch to next Tuesday, because like the payment tests are still failing, and uh we don't want to ship that broken."
  [email]="hey can you send me the numbers from last week, i need them for the board deck by tomorrow morning, thanks."
  [meeting]="Notes from the standup. Maria finished the login page and starts on billing today. Diego is blocked on the staging database and needs access from DevOps. Sofia fixed the popup paste bug and will review the demo script. The release is still planned for Friday."
)

piper() { uvx -q -p 3.12 --from piper-tts "$@"; }

mkdir -p "$CACHE"
if [[ ! -f "$CACHE/$VOICE.onnx" ]]; then
  echo "Downloading the $VOICE voice..."
  (cd "$CACHE" && piper python -m piper.download_voices "$VOICE")
fi
for name in "${!LINES[@]}"; do
  wav="$CACHE/$name.wav"
  txt="$CACHE/$name.txt"
  # Regenerate when the line's text changes.
  if [[ ! -f "$wav" || "$(cat "$txt" 2>/dev/null)" != "${LINES[$name]}" ]]; then
    printf '%s' "${LINES[$name]}" >"$txt"
    piper piper -m "$VOICE" --data-dir "$CACHE" -f "$wav" <"$txt"
  fi
done

# ---- Virtual microphone ---------------------------------------------------

OLD_SOURCE="$(pactl get-default-source)"
SINK_MODULE="$(pactl load-module module-null-sink sink_name=$SINK sink_properties=device.description=RatonDemo)"
MIC_MODULE="$(pactl load-module module-remap-source master=$SINK.monitor source_name=${SINK}_mic source_properties=device.description=RatonDemoMic)"
pactl set-default-source "${SINK}_mic"

APP_PID=""
EDITOR_PID=""
AUDIO_PID=""
VIDEO_PID=""
cleanup() {
  [[ -n "$VIDEO_PID" ]] && kill -INT "$VIDEO_PID" 2>/dev/null && wait "$VIDEO_PID" || true
  [[ -n "$AUDIO_PID" ]] && kill "$AUDIO_PID" 2>/dev/null && wait "$AUDIO_PID" || true
  [[ -n "$APP_PID" ]] && kill "$APP_PID" 2>/dev/null || true
  [[ -n "$EDITOR_PID" ]] && kill "$EDITOR_PID" 2>/dev/null || true
  pactl set-default-source "$OLD_SOURCE" || true
  pactl unload-module "$MIC_MODULE" || true
  pactl unload-module "$SINK_MODULE" || true
}
trap cleanup EXIT

# ---- Helpers --------------------------------------------------------------

# Speaks a clip into the virtual mic between two presses of the given toggle.
dictate() {
  local flag="$1" clip="$2"
  "$APP" "$flag"
  sleep 1
  paplay --device="$SINK" "$CACHE/$clip.wav"
  sleep 0.7
  "$APP" "$flag"
}

# The main window starts hidden, so the only visible window with this title is
# the popup. X11 window managers sometimes refuse it focus, so activate it here.
wait_popup() {
  local win
  for _ in $(seq 120); do
    win="$(xdotool search --onlyvisible --name "^$POPUP_TITLE\$" 2>/dev/null | head -1 || true)"
    if [[ -n "$win" ]]; then
      xdotool windowactivate --sync "$win"
      return
    fi
    sleep 0.5
  done
  echo "the popup didn't open" >&2
  exit 1
}

# Popup shortcuts: Ctrl+N runs the Nth action, in the order shown in the popup.
PROOFREAD=1 FRIENDLY=3 PROFESSIONAL=4 KEY_POINTS=7 TABLE=8
action() { xdotool key --clearmodifiers "ctrl+$1"; sleep "$WAIT_ACTION"; }

type_line() { xdotool type --delay 40 "$1"; xdotool key Return; }

# Fills the screen with the editor so the recording shows only the demo, and
# parks the pointer in it: without a caret position the popup opens there.
show_editor() {
  local win
  win="$(xdotool search --sync --onlyvisible --name "$(basename "$NOTES")" | head -1)"
  xdotool windowactivate --sync "$win" windowmove "$win" 0 0 windowsize "$win" 100% 100%
  xdotool mousemove --window "$win" 300 250
}

# ---- Demo -----------------------------------------------------------------

# A dev build finds libtranscribe here; an installed build has it on its rpath.
export LD_LIBRARY_PATH="$ROOT/src-tauri/transcribe-libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
"$APP" --start-hidden >/dev/null 2>&1 &
APP_PID=$!
sleep 8
kill -0 "$APP_PID" 2>/dev/null || { echo "the app exited at startup; run $APP to see why" >&2; exit 1; }

NOTES="$(mktemp --suffix=.md -t raton-demo-XXXX)"
"$EDITOR_CMD" "$NOTES" >/dev/null 2>&1 &
EDITOR_PID=$!
sleep 2
show_editor

if [[ -n "$RECORD" ]]; then
  parecord --device="$SINK.monitor" --file-format=wav "$RECORD.audio.wav" &
  AUDIO_PID=$!
  ffmpeg -loglevel error -y -f x11grab -framerate 30 -i "$DISPLAY" \
    -c:v libx264 -preset veryfast -pix_fmt yuv420p "$RECORD.video.mp4" &
  VIDEO_PID=$!
  sleep 1
fi

echo "1/3 Dictation, cleaned up by the LLM"
type_line "# 1. Dictation with LLM cleanup"
dictate --toggle-post-process dictation
sleep "$WAIT_LLM"
xdotool key Return Return

echo "2/3 Writing-tools popup"
type_line "# 2. Writing-tools popup"
dictate --toggle-popup email
wait_popup
sleep 1
action "$PROOFREAD"
action "$PROFESSIONAL"
action "$FRIENDLY"
# From the Final box, Shift+Tab reaches the custom instruction field.
xdotool key shift+Tab
xdotool type --delay 40 "Translate it to Argentine Spanish"
xdotool key Return
sleep "$WAIT_ACTION"
xdotool key ctrl+Return
sleep 1
xdotool key Return Return

echo "3/3 Notes into key points and a table"
type_line "# 3. Key points and table"
dictate --toggle-popup meeting
wait_popup
sleep 1
action "$KEY_POINTS"
action "$TABLE"
xdotool key ctrl+Return
sleep 3
xdotool key ctrl+s
sleep 1

if [[ -n "$RECORD" ]]; then
  kill -INT "$VIDEO_PID"; wait "$VIDEO_PID" || true; VIDEO_PID=""
  kill "$AUDIO_PID"; wait "$AUDIO_PID" || true; AUDIO_PID=""
  ffmpeg -loglevel error -y -i "$RECORD.video.mp4" -i "$RECORD.audio.wav" \
    -c:v copy -c:a aac -shortest "$RECORD"
  rm -f "$RECORD.video.mp4" "$RECORD.audio.wav"
  echo "Recorded $RECORD"
fi
echo "Done. The pasted results are in $NOTES"
