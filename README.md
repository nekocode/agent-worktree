# agent-worktree

A Git worktree workflow tool for AI coding agents. Enables parallel development with isolated environments.

[中文文档](README.zh-CN.md)

## Why

AI coding agents work best with isolated environments:

- **Parallel execution**: Run multiple agents simultaneously without interference
- **Clean separation**: Each feature gets its own working directory
- **Snap mode**: "Use and discard" workflow — create worktree, run agent, merge, cleanup

## Install

```bash
npm install -g agent-worktree
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

# List all worktrees
wt ls

# Switch to another worktree
wt cd feature-y

# Return to main repository
wt main

# Merge and cleanup
wt merge
```

## Snap Mode

One-liner for AI agent workflows:

```bash
wt new -s claude           # Random branch name
wt new fix-bug -s cursor   # Specified branch name
wt new -s "aider --model sonnet"  # Command with arguments
```

Flow: Create worktree → Enter → Run agent → [Develop] → Agent exits → Check changes → Merge → Cleanup

When the agent exits normally with uncommitted changes:
```
[c] Run git commit, then merge
[r] Reopen agent to continue
[x] Discard changes and exit
```

## Commands

### Worktree Management

| Command | Description |
|---------|-------------|
| `wt new [branch]` | Create worktree (random name if omitted) |
| `wt new --base <ref>` | Create from specific commit/branch |
| `wt new -s <cmd>` | Create + snap mode |
| `wt cd <branch>` | Switch to worktree |
| `wt ls` | List worktrees |
| `wt main` | Return to main repository |
| `wt mv <old> <new>` | Rename worktree (use `.` for current) |
| `wt rm <branch>` | Remove worktree (use `.` for current) |
| `wt rm -f <branch>` | Force remove with uncommitted changes |
| `wt clean` | Remove worktrees with no diff from trunk |

### Workflow

| Command | Description |
|---------|-------------|
| `wt merge` | Merge current worktree to trunk |
| `wt merge -s <strategy>` | Merge with strategy (squash/merge/rebase) |
| `wt merge --into <branch>` | Merge to specific branch |
| `wt merge -k` | Keep worktree after merge |
| `wt merge --continue` | Continue after resolving conflicts |
| `wt merge --abort` | Abort merge |
| `wt sync` | Sync updates from trunk (rebase) |
| `wt sync -s merge` | Sync with merge strategy |
| `wt sync --continue` | Continue after resolving conflicts |
| `wt sync --abort` | Abort sync |

### Configuration

| Command | Description |
|---------|-------------|
| `wt setup` | Install shell integration (auto-detect) |
| `wt setup --shell zsh` | Install for specific shell |
| `wt init` | Initialize project config |
| `wt init --trunk <branch>` | Initialize with specific trunk branch |

## Configuration

### Global Config `~/.agent-worktree/config.toml`

```toml
[general]
merge_strategy = "squash"  # squash | merge | rebase
copy_files = ["*.secret.*"]  # Gitignore-style patterns for files to copy

[hooks]
post_create = []
pre_merge = []
post_merge = []
```

### Project Config `.agent-worktree.toml`

```toml
[general]
trunk = "main"  # Trunk branch (auto-detected if omitted)
copy_files = [".env", ".env.*"]  # *.md for all, /*.md for root only

[hooks]
post_create = ["pnpm install"]
pre_merge = ["pnpm test", "pnpm lint"]
```

## Storage Layout

```
~/.agent-worktree/
├── config.toml                    # Global config
└── workspaces/
    └── {project}/
        ├── swift-fox.status.toml  # Worktree metadata
        ├── swift-fox/             # Worktree directory
        └── ...
```

## License

MIT
