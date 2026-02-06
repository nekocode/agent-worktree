// ============================================================
// Postinstall: Verify Platform Package
// ============================================================

const PLATFORMS = {
  "darwin-arm64": "@nekocode/agent-worktree-darwin-arm64",
  "darwin-x64": "@nekocode/agent-worktree-darwin-x64",
  "linux-x64": "@nekocode/agent-worktree-linux-x64",
  "win32-x64": "@nekocode/agent-worktree-win32-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  console.warn(`[agent-worktree] Warning: Unsupported platform ${key}`);
  console.warn(`[agent-worktree] Supported: ${Object.keys(PLATFORMS).join(", ")}`);
  process.exit(0);
}

try {
  require.resolve(`${pkg}/package.json`);
} catch {
  console.warn(`[agent-worktree] Warning: Platform package ${pkg} not installed`);
  console.warn(`[agent-worktree] This may happen if npm failed to install optional dependencies`);
  process.exit(0);
}

console.log("[agent-worktree] Installed successfully. Run 'wt setup' to enable shell integration.");
