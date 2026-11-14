#!/bin/sh
# Record the 100x30 catalog preview, asciiverse/bellbeat.cast, with hub's
# castrec.py: the demo playing Escape Sequence on Hard (into its chorus: the
# melody, its chords and the snare), muted so recording plays nothing
# aloud, drawn at 30 fps to keep the file small. The demo skips the kitty keyboard query,
# which a recorder never answers.
set -e
cd "$(dirname "$0")/.."
here=$(pwd)
castrec=$(cd ../hub/tools 2>/dev/null || cd ../../hub/tools; pwd)/castrec.py
cargo build --release
tmp=$(mktemp -d)
cp target/release/bellbeat "$tmp/"
(cd "$tmp" && PATH="$tmp:$PATH" HOME="$tmp" BELLBEAT_STATE="$tmp/state" \
  python3 "$castrec" "$here/asciiverse/bellbeat.cast" . "w38;kesc;w0.6;kq;w0.4" \
  -- bellbeat --demo --mute --fps 30 --song escape-sequence --difficulty hard)
rm -rf "$tmp"
