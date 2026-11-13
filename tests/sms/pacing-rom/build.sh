#!/usr/bin/env bash
set -euo pipefail

readonly DEVKIT_SMS_REVISION="561bc2cb9829cdf2e31ad8cd43835037c8c44a63"
readonly REQUIRED_SDCC_VERSION="4.6.0"

: "${DEVKIT_SMS_ROOT:?Set DEVKIT_SMS_ROOT to the checked-out sverx/devkitSMS repository}"
: "${OUT_DIR:?Set OUT_DIR to an output directory outside the repository}"
SDCC_BIN="${SDCC_BIN:-sdcc}"
CC_BIN="${CC_BIN:-cc}"

require_revision() {
    local actual
    actual="$(git -C "$DEVKIT_SMS_ROOT" rev-parse HEAD)"
    if [[ "$actual" != "$DEVKIT_SMS_REVISION" ]]; then
        printf 'Unexpected devkitSMS revision: expected %s, got %s\n' \
            "$DEVKIT_SMS_REVISION" "$actual" >&2
        return 1
    fi
}

SDCC_PATH="$(command -v "$SDCC_BIN")"
SDCC_VERSION_OUTPUT="$("$SDCC_PATH" --version 2>&1)"
if ! grep -Fq "$REQUIRED_SDCC_VERSION" <<<"$SDCC_VERSION_OUTPUT"; then
    printf 'Frame pacing ROM requires SDCC %s; detected:\n%s\n' \
        "$REQUIRED_SDCC_VERSION" "$SDCC_VERSION_OUTPUT" >&2
    exit 2
fi

require_revision

CRT0="$DEVKIT_SMS_ROOT/crt0/crt0_sms.rel"
PEEP_RULES="$DEVKIT_SMS_ROOT/SMSlib/src/peep-rules.txt"
MAKESMS_SOURCE="$DEVKIT_SMS_ROOT/tools/makesms/src/makesms.c"
SOURCE="$(cd "$(dirname "$0")" && pwd)/src/main.c"

for required_file in "$CRT0" "$PEEP_RULES" "$MAKESMS_SOURCE" "$SOURCE"; do
    if [[ ! -f "$required_file" ]]; then
        printf 'Required build input is missing: %s\n' "$required_file" >&2
        exit 2
    fi
done

mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
BUILD_DIR="$OUT_DIR/work"
mkdir -p "$BUILD_DIR"

"$CC_BIN" -std=c11 -D_DEFAULT_SOURCE -O2 -Wall -Wextra \
    "$MAKESMS_SOURCE" -o "$OUT_DIR/makesms"

for profile in 0 1 2 3; do
    case "$profile" in
        0) name="cpu-light" ;;
        1) name="cpu-fixed" ;;
        2) name="vdp-sprites" ;;
        3) name="combined" ;;
    esac

    profile_dir="$BUILD_DIR/$name"
    mkdir -p "$profile_dir"

    (
        cd "$profile_dir"
        "$SDCC_PATH" -c -mz80 --peep-file "$PEEP_RULES" \
            -DPACING_PROFILE="$profile" "$SOURCE"
        # crt0_sms.rel expects the SMSlib core globals and ISR entry points.
        # Compile the pinned SMSlib core instead of suppressing unresolved symbols.
        "$SDCC_PATH" -c -mz80 --max-allocs-per-node 100000 \
            --peep-file "$PEEP_RULES" \
            "$DEVKIT_SMS_ROOT/SMSlib/src/SMSlib.c"
        "$SDCC_PATH" -o "$OUT_DIR/$name.ihx" -mz80 --no-std-crt0 \
            --data-loc 0xC000 "$CRT0" main.rel SMSlib.rel
    )

    "$OUT_DIR/makesms" "$OUT_DIR/$name.ihx" "$OUT_DIR/$name.sms"

    rom_size="$(wc -c < "$OUT_DIR/$name.sms" | tr -d ' ')"
    if (( rom_size == 0 || rom_size % 16384 != 0 )); then
        printf '%s: invalid SMS ROM size: %s bytes\n' "$name" "$rom_size" >&2
        exit 1
    fi
    if (( rom_size > 16384 )); then
        printf '%s: expected one 16 KiB bank, got %s bytes\n' "$name" "$rom_size" >&2
        exit 1
    fi

    rom_sha256="$(sha256sum "$OUT_DIR/$name.sms" | awk '{print $1}')"
    ihx_sha256="$(sha256sum "$OUT_DIR/$name.ihx" | awk '{print $1}')"
    {
        printf 'profile=%s\n' "$name"
        printf 'profile_value=%s\n' "$profile"
        printf 'devkitsms_revision=%s\n' "$DEVKIT_SMS_REVISION"
        printf 'sdcc_version=%s\n' "$REQUIRED_SDCC_VERSION"
        printf 'rom_size=%s\n' "$rom_size"
        printf 'ihx_sha256=%s\n' "$ihx_sha256"
        printf 'rom_sha256=%s\n' "$rom_sha256"
    } > "$OUT_DIR/$name.build-info.txt"

    printf '%s %s bytes sha256=%s\n' "$name" "$rom_size" "$rom_sha256"
done

printf 'Built four SMS frame pacing ROM profiles in %s\n' "$OUT_DIR"
