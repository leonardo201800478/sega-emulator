#!/usr/bin/env bash
set -euo pipefail

# Rebuilds the two pinned SMS diagnostic ROMs locally.
# The generated ROMs stay outside Git and are written to test-results/homebrew-roms.
ROOT="$(git rev-parse --show-toplevel)"
OUT_DIR="${1:-$ROOT/test-results/homebrew-roms}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing dependency: $1" >&2
    exit 1
  }
}

for tool in git cmake make sdcc python3 sha256sum; do need "$tool"; done

SDCC_VERSION="$(sdcc --version | head -n 1)"
case "$SDCC_VERSION" in
  *4.5.0*) ;;
  *)
    echo "Expected SDCC 4.5.0; found: $SDCC_VERSION" >&2
    exit 1
    ;;
esac

clone_at() {
  local repo="$1" commit="$2" dest="$3"
  git clone --quiet "https://github.com/$repo.git" "$dest"
  git -C "$dest" checkout --quiet --detach "$commit"
  test "$(git -C "$dest" rev-parse HEAD)" = "$commit"
}

mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"

echo "[1/2] Building SmsVdpTest..."
clone_at FluBBaOfWard/SmsVdpTest 490c00b38c83012dee5a7a582097f080c210a3e7 "$WORK/SmsVdpTest"
clone_at vhelin/wla-dx 91c52b1f4ef3cc8ba3c0638f7536539579af6a9f "$WORK/wla-dx"

cmake -S "$WORK/wla-dx" -B "$WORK/wla-dx/build" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$WORK/wla-install"
cmake --build "$WORK/wla-dx/build" --config Release --parallel 2
cmake --install "$WORK/wla-dx/build"

(
  cd "$WORK/SmsVdpTest"
  "$WORK/wla-install/bin/wla-z80" -o object.o testmain.asm
  printf '[objects]\nobject.o\n' > linkfile
  "$WORK/wla-install/bin/wlalink" -d -r -v linkfile "$OUT_DIR/SmsVdpTest.sms"
)
python3 - "$OUT_DIR/SmsVdpTest.sms" <<'PY'
from pathlib import Path
import sys
rom = Path(sys.argv[1]).read_bytes()
assert len(rom) == 32768, f"Unexpected SmsVdpTest size: {len(rom)}"
PY
echo "SmsVdpTest SHA-256:"
sha256sum "$OUT_DIR/SmsVdpTest.sms"

echo "[2/2] Building SMS Test Suite 0.38..."
clone_at sverx/SMSTestSuite 985bc04fef6f3dc3c3450fbc84242676f241d316 "$WORK/SMSTestSuite"
clone_at sverx/devkitSMS 561bc2cb9829cdf2e31ad8cd43835037c8c44a63 "$WORK/devkitSMS"
clone_at joffb/banjo 7aaf837ebbe67e567e019b5819b5467bf62253ad "$WORK/banjo"

cp -a "$WORK/devkitSMS/SMSlib" "$WORK/SMSlib"
cp -a "$WORK/devkitSMS/PSGlib" "$WORK/PSGlib"
cp -a "$WORK/devkitSMS/crt0" "$WORK/crt0"
cp "$WORK/devkitSMS/SMSlib/src/peep-rules.txt" "$WORK/SMSlib/peep-rules.txt"
cp "$WORK/devkitSMS/SMSlib/src/SMSlib.h" "$WORK/SMSlib/SMSlib.h"
cp "$WORK/devkitSMS/PSGlib/src/PSGlib.h" "$WORK/PSGlib/PSGlib.h"
mkdir -p "$WORK/banjo_lib_minimal"
cp "$WORK/banjo/music_driver_sdas/banjo.h" "$WORK/banjo_lib_minimal/banjo.h"
cp "$WORK/banjo/lib/devkitsms/"*.rel "$WORK/banjo_lib_minimal/"

test -f "$WORK/SMSTestSuite/sn_opll_loop.rel"
test -f "$WORK/banjo_lib_minimal/banjo.rel"
test -f "$WORK/banjo_lib_minimal/banjo_opll.rel"
test -f "$WORK/banjo_lib_minimal/banjo_sn.rel"

make -C "$WORK/SMSlib/src" CONFIG="-DMD_PAD_SUPPORT -DVDPTYPE_DETECTION" SMSlib.lib
cp "$WORK/SMSlib/src/SMSlib.lib" "$WORK/SMSlib/SMSlib_MD_VDP.lib"
make -C "$WORK/PSGlib/src" PSGlib.lib
cp "$WORK/PSGlib/src/PSGlib.lib" "$WORK/PSGlib/PSGlib.lib"

(
  cd "$WORK/SMSTestSuite"
  python3 "$WORK/devkitSMS/tools/assets2banks/src/assets2banks.py" assets --firstbank=2 --singleheader=bank1.h
  sdcc -c -mz80 --peep-file "$WORK/SMSlib/peep-rules.txt" -o main.rel main.c
  sdcc -c -mz80 --constseg BANK2 -o bank2.rel bank2.c
  sdcc -c -mz80 --constseg BANK3 -o bank3.rel bank3.c
  sdcc -o output.ihx -mz80 --data-loc 0xC000 --no-std-crt0 \
    -Wl-b_BANK2=0x28000 -Wl-b_BANK3=0x38000 -L "$WORK/SMSlib" \
    "$WORK/crt0/crt0_sms.rel" main.rel "$WORK/SMSlib/SMSlib_MD_VDP.lib" \
    "$WORK/PSGlib/PSGlib.lib" "$WORK/banjo_lib_minimal/banjo.rel" \
    "$WORK/banjo_lib_minimal/banjo_opll.rel" "$WORK/banjo_lib_minimal/banjo_sn.rel" \
    sn_opll_loop.rel bank2.rel bank3.rel
  chmod +x "$WORK/devkitSMS/tools/makesms/Linux/makesms"
  SOURCE_DATE_EPOCH=1767225600 "$WORK/devkitSMS/tools/makesms/Linux/makesms" \
    output.ihx "$OUT_DIR/SMS-Test-Suite-0.38.sms"
)

python3 - "$OUT_DIR/SMS-Test-Suite-0.38.sms" <<'PY'
from pathlib import Path
import sys
rom = Path(sys.argv[1]).read_bytes()
assert len(rom) == 65536, f"Unexpected SMS Test Suite size: {len(rom)}"
assert rom[0x7FF0:0x7FF8] == b"TMR SEGA", "Missing TMR SEGA header"
PY

echo "SMS Test Suite 0.38 SHA-256:"
sha256sum "$OUT_DIR/SMS-Test-Suite-0.38.sms"

echo
echo "ROMs generated in: $OUT_DIR"
echo "Expected hashes:"
echo "SmsVdpTest:         14e52a545a2a77bdd7f0ec1e52d78bd1abb3f64dd269f1f4639bbc8b2608c2f2"
echo "SMS Test Suite 0.38: 4ef4f1865772cc1efdc69a7ec23cd5da0f7784ef2b9a08a86c7ab7bf4e1464ef"
