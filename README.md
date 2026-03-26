# agent-worktree

[![npm version](https://img.shields.io/npm/v/agent-worktree)](https://www.npmjs.com/package/agent-worktree)

A Git worktree workflow tool for AI coding agents. Enables parallel development with isolated environments.

[中文文档](README.zh-CN.md)

![Cover](cover.jpg)

## Why

AI coding agents work best with isolated environments:

- **Parallel execution**: Run multiple agents simultaneously without interference
- **Clean separation**: Each feature gets its own working directory
- **Snap mode**: "Use and discard" workflow — create worktree, run agent, merge, cleanup

## Install

```bash
npm install -g agent-worktree
```

Update to the latest version:

```bash
wt update
```

Shell integration is installed automatically. To reinstall manually:

```bash
wt setup
```

Supported shells: bash, zsh, fish, PowerShell

## Quick Start

```bash
# Create a worktree and enter it
wt new feature-x

# ... develop, commit ...

# Merge back (merges to the branch you were on when creating)
wt merge            # keeps worktree
wt merge -d         # deletes worktree after merge
```

Other useful commands:

```bash
wt ls              # List all worktrees (with BASE branch info)
wt cd feature-y    # Switch to another worktree
wt main            # Return to main repository
```

## Snap Mode

One-liner for AI agent workflows:

```bash
wt new -s claude           # Random branch name
wt new fix-bug -s codex    # Specified branch name
wt new -s "claude --dangerously-skip-permissions"  # Command with arguments
```

Flow: Create worktree → Enter → Run agent → [Develop] → Agent exits → Check changes → Merge → Cleanup

When the agent exits normally:

- **No changes**: Worktree cleaned up automatically
- **Only commits** (nothing uncommitted):
  ```
  [m] Merge into base branch
  [q] Exit snap mode
  ```
- **Uncommitted changes**:
  ```
  [r] Reopen agent (let agent commit)
  [q] Exit snap mode (commit manually)
  ```

## Commands

### Worktree Management

| Command | Description |
|---------|-------------|
| `wt new [branch]` | Create worktree from current branch (random name if omitted) |
| `wt new --base <branch>` | Create from specific base branch (default: current branch) |
| `wt new -s <cmd>` | Create + snap mode |
| `wt cd <branch>` | Switch to worktree |
| `wt ls` | List worktrees |
| `wt ls -l` | Show full path for each worktree |
| `wt main` | Return to main repository |
| `wt mv <old> <new>` | Rename worktree (use `.` for current) |
| `wt rm <branch>` | Remove worktree (use `.` for current) |
| `wt rm -f <branch>` | Force remove with uncommitted changes |
| `wt clean` | Remove worktrees with no diff from trunk |
| `wt clean --dry-run` | Preview which worktrees would be cleaned |

### Workflow

| Command | Description |
|---------|-------------|
| `wt merge` | Merge to base branch (falls back to trunk, default: squash) |
| `wt merge -s <strategy>` | Merge with strategy (squash/merge) |
| `wt merge --into <branch>` | Merge to specific branch (overrides base) |
| `wt merge -d` | Delete worktree after merge (default: keep) |
| `wt merge -H` | Skip pre-merge hooks |
| `wt sync` | Sync from base branch (falls back to trunk, default: rebase) |
| `wt sync -s <strategy>` | Sync with strategy (rebase/merge) |
| `wt sync --from <branch>` | Sync from specific branch (overrides base) |
| `wt sync --continue` | Continue after resolving conflicts |
| `wt sync --abort` | Abort sync |

### Info

| Command | Description |
|---------|-------------|
| `wt status` | Show current worktree information |
| `wt update` | Update to the latest version |

### Configuration

| Command | Description |
|---------|-------------|
| `wt setup` | Install shell integration (auto-detect) |
| `wt setup --shell zsh` | Install for specific shell |
| `wt init` | Initialize project config |
| `wt init --trunk <branch>` | Initialize with specific trunk branch |
| `wt init --merge-strategy <strategy>` | Set default merge strategy (squash/merge) |
| `wt init --copy-files <pattern>` | Files to copy to new worktrees (repeatable) |

## Configuration

### Global Config `~/.agent-worktree/config.toml`

```toml
[general]
merge_strategy = "squash"  # squash | merge
trunk = "main"  # Trunk branch (auto-detected if omitted)
copy_files = [".env", ".env.*"]  # Gitignore-style patterns for files to copy

[hooks]
post_create = ["pnpm install"]
pre_merge = ["pnpm test", "pnpm lint"]
post_merge = []
```

### Project Config `.agent-worktree.toml`

```toml
[general]
copy_files = ["*.secret.*"]
# ...
```

## Storage Layout

```
~/.agent-worktree/
├── config.toml                    # Global config
└── workspaces/
    └── {project}/
        ├── swift-fox.toml         # Worktree metadata
        ├── swift-fox/             # Worktree directory
        └── ...
```

## License

MIT
