#!/bin/bash
# ============================================================
# Build npm packages with precompiled binaries
# ============================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
NPM_DIR="$PROJECT_ROOT/npm"

# ============================================================
# Helpers
# ============================================================

log() {
    echo "[build-npm] $1"
}

error() {
    echo "[build-npm] ERROR: $1" >&2
    exit 1
}

# ============================================================
# Platform Detection
# ============================================================

detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        darwin) os="darwin" ;;
        linux) os="linux" ;;
        *) error "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        arm64|aarch64) arch="arm64" ;;
        x86_64|amd64) arch="x64" ;;
        *) error "Unsupported arch: $arch" ;;
    esac

    echo "${os}-${arch}"
}

# ============================================================
# Build
# ============================================================

build_current_platform() {
    local platform=$(detect_platform)
    local target_dir="$NPM_DIR/agent-worktree-$platform/bin"

    log "Building for $platform..."

    cd "$PROJECT_ROOT"
    cargo build --release

    log "Copying binary to $target_dir"
    mkdir -p "$target_dir"
    cp target/release/wt "$target_dir/wt"
    chmod +x "$target_dir/wt"

    log "Done: agent-worktree-$platform"
}

build_cross_compile() {
    local target=$1
    local npm_pkg=$2
    local is_windows=$3

    log "Cross-compiling for $target..."

    cd "$PROJECT_ROOT"

    if ! command -v cross &> /dev/null; then
        log "Installing cross..."
        cargo install cross
    fi

    cross build --release --target "$target"

    local target_dir="$NPM_DIR/$npm_pkg/bin"
    mkdir -p "$target_dir"

    if [[ "$is_windows" == "true" ]]; then
        cp "target/$target/release/wt.exe" "$target_dir/wt.exe"
    else
        cp "target/$target/release/wt" "$target_dir/wt"
        chmod +x "$target_dir/wt"
    fi

    log "Done: $npm_pkg"
}

# ============================================================
# Main
# ============================================================

case "${1:-current}" in
    current)
        build_current_platform
        ;;
    all)
        log "Building all platforms (requires cross)..."
        build_cross_compile "aarch64-apple-darwin" "agent-worktree-darwin-arm64"
        build_cross_compile "x86_64-apple-darwin" "agent-worktree-darwin-x64"
        build_cross_compile "x86_64-unknown-linux-gnu" "agent-worktree-linux-x64"
        build_cross_compile "x86_64-pc-windows-gnu" "agent-worktree-win32-x64" "true"
        ;;
    darwin-arm64)
        build_cross_compile "aarch64-apple-darwin" "agent-worktree-darwin-arm64"
        ;;
    darwin-x64)
        build_cross_compile "x86_64-apple-darwin" "agent-worktree-darwin-x64"
        ;;
    linux-x64)
        build_cross_compile "x86_64-unknown-linux-gnu" "agent-worktree-linux-x64"
        ;;
    win32-x64)
        build_cross_compile "x86_64-pc-windows-gnu" "agent-worktree-win32-x64" "true"
        ;;
    *)
        echo "Usage: $0 [current|all|darwin-arm64|darwin-x64|linux-x64|win32-x64]"
        exit 1
        ;;
esac

log "Build complete!"
