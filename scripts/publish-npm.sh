#!/bin/bash
# ============================================================
# Publish npm packages
# ============================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
NPM_DIR="$PROJECT_ROOT/npm"

# ============================================================
# Helpers
# ============================================================

log() {
    echo "[publish-npm] $1"
}

error() {
    echo "[publish-npm] ERROR: $1" >&2
    exit 1
}

# ============================================================
# Version Sync
# ============================================================

sync_versions() {
    local version=$1

    log "Syncing version to $version..."

    for pkg_dir in "$NPM_DIR"/agent-worktree*/; do
        local pkg_json="$pkg_dir/package.json"
        if [[ -f "$pkg_json" ]]; then
            # macOS sed needs different syntax
            if [[ "$(uname)" == "Darwin" ]]; then
                sed -i '' "s/\"version\": \".*\"/\"version\": \"$version\"/" "$pkg_json"
            else
                sed -i "s/\"version\": \".*\"/\"version\": \"$version\"/" "$pkg_json"
            fi
            log "Updated $(basename "$pkg_dir")"
        fi
    done

    # Also update optionalDependencies versions in main package
    local main_pkg="$NPM_DIR/agent-worktree/package.json"
    for platform in darwin-arm64 darwin-x64 linux-x64 win32-x64; do
        if [[ "$(uname)" == "Darwin" ]]; then
            sed -i '' "s/\"@nekocode\/agent-worktree-$platform\": \".*\"/\"@nekocode\/agent-worktree-$platform\": \"$version\"/" "$main_pkg"
        else
            sed -i "s/\"@nekocode\/agent-worktree-$platform\": \".*\"/\"@nekocode\/agent-worktree-$platform\": \"$version\"/" "$main_pkg"
        fi
    done
}

# ============================================================
# Registry Check
# ============================================================

is_published() {
    local pkg_name=$1
    local version=$2
    npm view "${pkg_name}@${version}" version &>/dev/null
}

# ============================================================
# Publish
# ============================================================

publish_package() {
    local pkg_dir=$1
    local pkg_name=$2
    local version=$3
    local dry_run=$4

    if is_published "$pkg_name" "$version"; then
        log "Skipping $pkg_name@$version (already published)"
        return 0
    fi

    log "Publishing $pkg_name@$version..."

    cd "$pkg_dir"

    if [[ "$dry_run" == "true" ]]; then
        npm publish --dry-run
    else
        npm publish --access public
    fi
}

# ============================================================
# Main
# ============================================================

VERSION=${1:-}
DRY_RUN=${2:-false}

if [[ -z "$VERSION" ]]; then
    # Read version from Cargo.toml
    VERSION=$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
    log "Using version from Cargo.toml: $VERSION"
fi

sync_versions "$VERSION"

# Publish platform packages first (main package depends on them)
for platform in darwin-arm64 darwin-x64 linux-x64 win32-x64; do
    pkg_dir="$NPM_DIR/agent-worktree-$platform"
    pkg_name="@nekocode/agent-worktree-$platform"
    # Windows uses .exe extension
    if [[ "$platform" == "win32-x64" ]]; then
        binary="$pkg_dir/bin/wt.exe"
    else
        binary="$pkg_dir/bin/wt"
    fi
    if [[ -f "$binary" && -s "$binary" ]]; then
        publish_package "$pkg_dir" "$pkg_name" "$VERSION" "$DRY_RUN"
    else
        log "Skipping $pkg_name (no binary found)"
    fi
done

# Publish main package last
publish_package "$NPM_DIR/agent-worktree" "agent-worktree" "$VERSION" "$DRY_RUN"

log "Publish complete!"
