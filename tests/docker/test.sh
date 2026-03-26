#!/usr/bin/env bash
# Docker verification for mdskim setup and external dependency management
# Usage: bash tests/docker/test.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_MD="$SCRIPT_DIR/test.md"
SCRIPTS_DIR="$PROJECT_DIR/scripts"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASS=0
FAIL=0
RESULTS=()

log_header() { echo -e "\n${BLUE}========================================${NC}"; echo -e "${BLUE}  $1${NC}"; echo -e "${BLUE}========================================${NC}"; }
log_pass()   { echo -e "  ${GREEN}✓ PASS${NC}: $1"; PASS=$((PASS + 1)); RESULTS+=("${GREEN}✓${NC} $1"); }
log_fail()   { echo -e "  ${RED}✗ FAIL${NC}: $1"; echo -e "    ${YELLOW}Expected${NC}: $2"; echo -e "    ${YELLOW}Actual${NC}:   $3"; FAIL=$((FAIL + 1)); RESULTS+=("${RED}✗${NC} $1"); }

if ! command -v docker &>/dev/null; then
    echo "Error: docker not found"; exit 1
fi

###############################################################################
# Step 0: Build images
###############################################################################
log_header "Step 0: Building Docker images"

# --- Builder: compile Linux binary ---
echo "Building mdskim binary..."
docker build -t mdskim-builder -f - "$PROJECT_DIR" <<'DOCKERFILE'
FROM rust:1-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY scripts/ scripts/
COPY assets/ assets/
RUN cargo build --release
DOCKERFILE
BINARY_DIR="$PROJECT_DIR/tmp/docker_bin"
mkdir -p "$BINARY_DIR"
docker run --rm -v "$BINARY_DIR:/out" mdskim-builder cp /src/target/release/mdskim /out/mdskim
chmod +x "$BINARY_DIR/mdskim"
BINARY="$BINARY_DIR/mdskim"
echo "Binary: $(du -h "$BINARY" | cut -f1)"

# Empty context dir for images that don't need files
EMPTY_CTX=$(mktemp -d)
trap 'rm -rf "$EMPTY_CTX" "$BINARY_DIR"' EXIT

# --- Image 1: Clean Debian (no Node) ---
echo "Building image: mdskim-test-clean..."
docker build -t mdskim-test-clean -f - "$EMPTY_CTX" <<'DOCKERFILE'
FROM debian:bookworm-slim
DOCKERFILE

# --- Image 2: Node only ---
echo "Building image: mdskim-test-node..."
docker build -t mdskim-test-node -f - "$EMPTY_CTX" <<'DOCKERFILE'
FROM node:22-slim
DOCKERFILE

# --- Image 3: Node + Chromium ---
echo "Building image: mdskim-test-full..."
docker build -t mdskim-test-full -f - "$EMPTY_CTX" <<'DOCKERFILE'
FROM node:22-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends chromium \
    && rm -rf /var/lib/apt/lists/*
DOCKERFILE

# --- Image 6: Node + Chromium (non-standard path) ---
echo "Building image: mdskim-test-chrome-moved..."
docker build -t mdskim-test-chrome-moved -f - "$EMPTY_CTX" <<'DOCKERFILE'
FROM node:22-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends chromium \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /opt/browser \
    && mv /usr/bin/chromium /opt/browser/chromium-custom \
    && rm -f /usr/bin/chromium-browser /usr/bin/google-chrome /usr/bin/google-chrome-stable
DOCKERFILE

echo "All images built."

###############################################################################
# Helper
###############################################################################
run_test() {
    local image="$1" test_cmd="$2"
    local tmpout tmperr
    tmpout=$(mktemp); tmperr=$(mktemp)

    DOCKER_EXIT=0
    docker run --rm \
        -e HOME=/root \
        -v "$BINARY:/usr/local/bin/mdskim:ro" \
        -v "$TEST_MD:/tmp/test.md:ro" \
        -v "$SCRIPTS_DIR:/opt/mdskim/scripts:ro" \
        "$image" \
        bash -c "$test_cmd" \
        >"$tmpout" 2>"$tmperr" || DOCKER_EXIT=$?

    DOCKER_STDOUT=$(cat "$tmpout")
    DOCKER_STDERR=$(cat "$tmperr")
    DOCKER_ALL="$DOCKER_STDOUT $DOCKER_STDERR"
    rm -f "$tmpout" "$tmperr"
}

assert_exit() {
    local expected="$1" desc="$2"
    [[ "$DOCKER_EXIT" == "$expected" ]] \
        && log_pass "$desc" \
        || log_fail "$desc" "exit $expected" "exit $DOCKER_EXIT | $(echo "$DOCKER_ALL" | tail -3)"
}

assert_contains() {
    local pattern="$1" desc="$2"
    echo "$DOCKER_ALL" | grep -qiE "$pattern" \
        && log_pass "$desc" \
        || log_fail "$desc" "output matches '$pattern'" "$(echo "$DOCKER_ALL" | tail -5)"
}

assert_file_ok() {
    local desc="$1"
    echo "$DOCKER_STDOUT" | grep -q "FILE_OK" \
        && log_pass "$desc" \
        || log_fail "$desc" "FILE_OK" "$(echo "$DOCKER_ALL" | tail -5)"
}

###############################################################################
# Pattern 1: Clean Debian (no Node.js)
###############################################################################
log_header "Pattern 1: Clean Debian — setup without Node.js"

run_test mdskim-test-clean "mdskim setup 2>&1"
assert_exit 1 "P1: setup exits with error"
assert_contains "node.js not found|node.*not found" "P1: error mentions Node.js"

###############################################################################
# Pattern 2: Node only (no Chromium)
###############################################################################
log_header "Pattern 2: Node.js only — setup + HTML export"

run_test mdskim-test-node "mdskim setup 2>&1"
assert_exit 0 "P2: setup succeeds"
assert_contains "installed successfully" "P2: shows install success"
assert_contains "chrome.*not found|chromium.*not found" "P2: warns about missing Chrome"

run_test mdskim-test-node "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-html /tmp/out.html 2>/dev/null; test -s /tmp/out.html && echo FILE_OK || echo FILE_MISSING"
assert_file_ok "P2: HTML export produces output"

run_test mdskim-test-node "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf 2>&1"
assert_exit 1 "P2: PDF fails without Chrome"
assert_contains "chrome|chromium" "P2: PDF error mentions Chrome"

###############################################################################
# Pattern 3: Node + Chromium (full features)
###############################################################################
log_header "Pattern 3: Node + Chromium — full functionality"

run_test mdskim-test-full "mdskim setup 2>&1"
assert_exit 0 "P3: setup succeeds"
assert_contains "installed successfully" "P3: shows install success"
assert_contains "chrome.*found|chromium.*found" "P3: detects Chrome/Chromium"

run_test mdskim-test-full "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-html /tmp/out.html 2>/dev/null; test -s /tmp/out.html && echo FILE_OK || echo FILE_MISSING"
assert_file_ok "P3: HTML export produces output"

run_test mdskim-test-full "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf --no-sandbox 2>/dev/null; test -s /tmp/out.pdf && echo FILE_OK || echo FILE_MISSING"
assert_file_ok "P3: PDF export with --no-sandbox produces output"

###############################################################################
# Pattern 4: Selective install (--math only)
###############################################################################
log_header "Pattern 4: Selective install — --math only"

run_test mdskim-test-full "mdskim setup --math 2>&1"
assert_exit 0 "P4: setup --math succeeds"
assert_contains "mathjax" "P4: installs mathjax-full"

run_test mdskim-test-full "mdskim setup --math >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf --no-sandbox 2>&1"
assert_exit 1 "P4: PDF fails without puppeteer-core"
assert_contains "puppeteer-core|setup --pdf" "P4: error suggests setup --pdf"

###############################################################################
# Pattern 5: Render without setup
###############################################################################
log_header "Pattern 5: Render without prior setup"

run_test mdskim-test-node "mdskim /tmp/test.md --export-pdf /tmp/out.pdf 2>&1"
assert_exit 1 "P5: PDF fails without setup"
assert_contains "puppeteer-core|setup" "P5: error guides user to run setup"

run_test mdskim-test-node "mdskim /tmp/test.md --export-html /tmp/out.html 2>/dev/null; test -s /tmp/out.html && echo FILE_OK || echo FILE_MISSING"
assert_file_ok "P5: HTML export works without setup (rendering skipped)"

###############################################################################
# Pattern 6: CHROME_PATH environment variable
###############################################################################
log_header "Pattern 6: CHROME_PATH with non-standard location"

# Without CHROME_PATH — standard paths removed, should fail
run_test mdskim-test-chrome-moved "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf --no-sandbox 2>&1"
assert_exit 1 "P6: PDF fails when Chrome not in standard path"
assert_contains "chrome|chromium|not found" "P6: error mentions Chrome not found"

# With CHROME_PATH — should succeed
run_test mdskim-test-chrome-moved "export CHROME_PATH=/opt/browser/chromium-custom; mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf --no-sandbox 2>/dev/null; test -s /tmp/out.pdf && echo FILE_OK || echo FILE_MISSING"
assert_file_ok "P6: PDF works with CHROME_PATH"

###############################################################################
# Pattern 7: Root + --no-sandbox
###############################################################################
log_header "Pattern 7: Root user + --no-sandbox requirement"

run_test mdskim-test-full "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf 2>&1"
assert_exit 1 "P7: PDF fails as root without --no-sandbox"
assert_contains "no-sandbox|sandbox" "P7: error mentions --no-sandbox"

run_test mdskim-test-full "mdskim setup >/dev/null 2>&1; mdskim /tmp/test.md --export-pdf /tmp/out.pdf --no-sandbox 2>/dev/null; test -s /tmp/out.pdf && echo FILE_OK || echo FILE_MISSING"
assert_file_ok "P7: PDF succeeds with --no-sandbox"

###############################################################################
# Summary
###############################################################################
echo ""
log_header "Results Summary"
echo ""
for r in "${RESULTS[@]}"; do
    echo -e "  $r"
done
echo ""
TOTAL=$((PASS + FAIL))
echo -e "  ${GREEN}Passed${NC}: $PASS / $TOTAL"
if [[ $FAIL -gt 0 ]]; then
    echo -e "  ${RED}Failed${NC}: $FAIL / $TOTAL"
    EXIT_CODE=1
else
    echo -e "  ${GREEN}All tests passed!${NC}"
    EXIT_CODE=0
fi

# Cleanup (handled by trap)
echo -e "\nCleaned up build artifacts."
exit $EXIT_CODE
