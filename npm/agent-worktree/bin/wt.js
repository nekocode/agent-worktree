#!/usr/bin/env node

const { execFileSync } = require("child_process");
const { join } = require("path");

// ============================================================
// Platform Detection
// ============================================================

const PLATFORMS = {
  "darwin-arm64": "@nekocode/agent-worktree-darwin-arm64",
  "darwin-x64": "@nekocode/agent-worktree-darwin-x64",
  "linux-x64": "@nekocode/agent-worktree-linux-x64",
  "win32-x64": "@nekocode/agent-worktree-win32-x64",
};

function getPlatformPackage() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PLATFORMS[key];
  if (!pkg) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(", ")}`);
    process.exit(1);
  }
  return pkg;
}

// ============================================================
// Binary Resolution
// ============================================================

function getBinaryPath() {
  const pkg = getPlatformPackage();
  const exe = process.platform === "win32" ? "wt.exe" : "wt";
  try {
    const pkgPath = require.resolve(`${pkg}/package.json`);
    return join(pkgPath, "..", "bin", exe);
  } catch {
    console.error(`Platform package not found: ${pkg}`);
    console.error("Try reinstalling: npm install -g agent-worktree");
    process.exit(1);
  }
}

// ============================================================
// Execute
// ============================================================

const binary = getBinaryPath();
const args = process.argv.slice(2);

try {
  execFileSync(binary, args, { stdio: "inherit" });
} catch (err) {
  if (err.status !== undefined) {
    process.exit(err.status);
  }
  if (err.code === "ENOENT") {
    console.error(`Binary not found: ${binary}`);
    console.error("Try reinstalling: npm install -g agent-worktree");
    process.exit(1);
  }
  if (err.code === "EACCES") {
    console.error(`Permission denied: ${binary}`);
    console.error("Try: chmod +x " + binary);
    process.exit(1);
  }
  console.error(`Failed to execute wt: ${err.message}`);
  process.exit(1);
}
