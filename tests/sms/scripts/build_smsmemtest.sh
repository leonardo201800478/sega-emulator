#!/usr/bin/env bash
set -euo pipefail

readonly SMSMEMTEST_REVISION="12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9"
readonly DEVKIT_SMS_REVISION="561bc2cb9829cdf2e31ad8cd43835037c8c44a63"
readonly REQUIRED_SDCC_VERSION="4.6.0"

: "${SMSMEMTEST_ROOT:?Set SMSMEMTEST_ROOT to the checked-out sverx/SMSmemtest repository}"
: "${DEVKIT_SMS_ROOT:?Set DEVKIT_SMS_ROOT to the checked-out sverx/devkitSMS repository}"
: "${OUT_DIR:?Set OUT_DIR to the output directory}"
SDCC_BIN="${SDCC_BIN:-sdcc}"
CC_BIN="${CC_BIN:-cc}"
MAKE_BIN="${MAKE_BIN:-make}"

require_revision() {
    local repository="$1"
    local expected_revision="$2"
    local actual_revision
    actual_revision="$(git -C "$repository" rev-parse HEAD)"
    if [[ "$actual_revision" != "$expected_revision" ]]; then
        printf 'Unexpected source revision in %s: expected %s, got %s\n' \
            "$repository" "$expected_revision" "$actual_revision" >&2
        return 1
    fi
}

SDCC_PATH="$(command -v "$SDCC_BIN")"
SDCC_VERSION_OUTPUT="$("$SDCC_PATH" --version 2>&1)"
if ! grep -Fq "$REQUIRED_SDCC_VERSION" <<<"$SDCC_VERSION_OUTPUT"; then
    printf 'SMSmemtest build requires SDCC %s; detected:\n%s\n' \
        "$REQUIRED_SDCC_VERSION" "$SDCC_VERSION_OUTPUT" >&2
    exit 2
fi

require_revision "$SMSMEMTEST_ROOT" "$SMSMEMTEST_REVISION"
require_revision "$DEVKIT_SMS_ROOT" "$DEVKIT_SMS_REVISION"

SMSLIB_DIR="$DEVKIT_SMS_ROOT/SMSlib/src"
SMSLIB_MAKEFILE="$SMSLIB_DIR/Makefile"
PEEP_RULES="$SMSLIB_DIR/peep-rules.txt"
CRT0_OBJECT="$SMSMEMTEST_ROOT/crt0_memtest.rel"
IHX2SMS_SOURCE="$DEVKIT_SMS_ROOT/tools/legacy/ihx2sms/src/ihx2sms.c"

for required_file in "$SMSLIB_MAKEFILE" "$PEEP_RULES" "$CRT0_OBJECT" "$IHX2SMS_SOURCE" "$SMSMEMTEST_ROOT/src/main.c"; do
    if [[ ! -f "$required_file" ]]; then
        printf 'Required upstream build input is missing: %s\n' "$required_file" >&2
        exit 2
    fi
done

SDCC_DIR="$(dirname "$SDCC_PATH")"
SDAR_BIN="$SDCC_DIR/sdar"
if [[ ! -x "$SDAR_BIN" ]]; then
    SDAR_BIN="$(command -v sdar || true)"
fi
if [[ -z "$SDAR_BIN" || ! -x "$SDAR_BIN" ]]; then
    echo "sdar was not found beside SDCC or on PATH" >&2
    exit 2
fi

mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
BUILD_DIR="$OUT_DIR/work"
mkdir -p "$BUILD_DIR"

# Build SMSlib from the pinned devkitSMS source with the same SDCC toolchain.
"$MAKE_BIN" -C "$SMSLIB_DIR" clean all CC="$SDCC_PATH" AR="$SDAR_BIN"

# Compile the pinned SMSmemtest source using devkitSMS' peephole rules and headers.
cp "$SMSMEMTEST_ROOT/src/main.c" "$BUILD_DIR/main.c"
cp "$CRT0_OBJECT" "$BUILD_DIR/crt0_memtest.rel"
(
    cd "$BUILD_DIR"
    "$SDCC_PATH" -c -mz80 -I"$SMSLIB_DIR" --peep-file "$PEEP_RULES" main.c
    "$SDCC_PATH" -o "$OUT_DIR/SMSmemtest.ihx" -mz80 --no-std-crt0 --data-loc 0xC000 \
        crt0_memtest.rel "$SMSLIB_DIR/SMSlib.lib" main.rel
)

# Build the public-domain converter from the same pinned devkitSMS revision.
"$CC_BIN" -std=c11 -O2 -Wall -Wextra "$IHX2SMS_SOURCE" -o "$OUT_DIR/ihx2sms"
"$OUT_DIR/ihx2sms" "$OUT_DIR/SMSmemtest.ihx" "$OUT_DIR/SMSmemtest.sms"

ROM_SIZE="$(wc -c < "$OUT_DIR/SMSmemtest.sms" | tr -d ' ')"
if [[ "$ROM_SIZE" != "32768" ]]; then
    printf 'Expected upstream SMSmemtest 32 KiB ROM, got %s bytes\n' "$ROM_SIZE" >&2
    exit 1
fi

ROM_SHA256="$(sha256sum "$OUT_DIR/SMSmemtest.sms" | awk '{print $1}')"
{
    printf 'smsmemtest_revision=%s\n' "$SMSMEMTEST_REVISION"
    printf 'devkitsms_revision=%s\n' "$DEVKIT_SMS_REVISION"
    printf 'sdcc_version=%s\n' "$REQUIRED_SDCC_VERSION"
    printf 'rom_size=%s\n' "$ROM_SIZE"
    printf 'rom_sha256=%s\n' "$ROM_SHA256"
} | tee "$OUT_DIR/SMSmemtest.build-info.txt"

printf 'Built SMSmemtest ROM: %s\n' "$OUT_DIR/SMSmemtest.sms"
