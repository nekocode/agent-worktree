// ============================================================
// Postinstall: Verify Platform Package & Install Shell Wrapper
// ============================================================

const { execFileSync } = require("child_process");
const { join } = require("path");

// ============================================================
// Platform Verification
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

let binaryPath;
try {
  const pkgPath = require.resolve(`${pkg}/package.json`);
  const exe = process.platform === "win32" ? "wt.exe" : "wt";
  binaryPath = join(pkgPath, "..", "bin", exe);
} catch {
  console.warn(`[agent-worktree] Warning: Platform package ${pkg} not installed`);
  console.warn(`[agent-worktree] This may happen if npm failed to install optional dependencies`);
  process.exit(0);
}

// ============================================================
// Shell Wrapper Installation
// ============================================================

try {
  console.log("[agent-worktree] Installing shell integration...");
  execFileSync(binaryPath, ["setup"], { stdio: "inherit" });
  if (process.platform === "win32") {
    console.log("[agent-worktree] Shell integration installed. Restart PowerShell to apply changes.");
  } else {
    console.log("[agent-worktree] Shell integration installed. Restart your shell or run: source ~/.bashrc (or ~/.zshrc)");
  }
} catch (err) {
  // Non-fatal: user can run `wt setup` manually
  console.warn("[agent-worktree] Warning: Could not install shell integration automatically");
  console.warn("[agent-worktree] Run 'wt setup' manually to enable shell integration");
}
