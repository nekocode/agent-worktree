# agent-worktree

[![npm version](https://img.shields.io/npm/v/agent-worktree)](https://www.npmjs.com/package/agent-worktree)

为 AI 编程 agent 设计的 Git worktree 工作流工具。提供隔离的并行开发环境。

[English](README.md)

![Cover](cover.jpg)

## 为什么需要

AI 编程 agent 在隔离环境中工作效果最佳：

- **并行执行**：同时运行多个 agent，互不干扰
- **环境隔离**：每个功能独立工作目录
- **Snap 模式**："即用即删"工作流 — 创建 worktree、运行 agent、合并、清理

## 安装

```bash
npm install -g agent-worktree
```

更新到最新版本：

```bash
wt update
```

Shell 集成会自动安装。如需手动重新安装：

```bash
wt setup
```

支持的 shell：bash、zsh、fish、PowerShell

## 快速开始

```bash
# 创建 worktree 并进入
wt new feature-x

# ... 开发、提交 ...

# 合并（默认 merge 回创建时所在的分支）
wt merge            # 保留 worktree
wt merge -d         # 合并后删除 worktree
```

其他常用命令：

```bash
wt ls              # 列出所有 worktree（含 BASE 分支信息）
wt cd feature-y    # 切换到另一个 worktree
wt main            # 返回主仓库
```

## Snap 模式

AI agent 工作流一行搞定：

```bash
wt new -s claude           # 随机分支名
wt new fix-bug -s codex    # 指定分支名
wt new -s "claude --dangerously-skip-permissions"  # 带参数的命令
```

流程：创建 worktree → 进入 → 运行 agent → [开发] → agent 退出 → 检查更改 → 合并 → 清理

Agent 正常退出时：

- **无改动**：自动清理 worktree
- **只有 commits**（无未提交更改）：
  ```
  [m] 合并到 base 分支
  [q] 退出 snap mode
  ```
- **有未提交更改**：
  ```
  [r] 重新打开 agent（让 agent 提交）
  [q] 退出 snap mode（手动提交）
  ```

## 命令

### Worktree 管理

| 命令 | 描述 |
|------|------|
| `wt new [branch]` | 从当前分支创建 worktree（省略则随机命名） |
| `wt new --base <branch>` | 指定 base 分支（默认为当前分支） |
| `wt new -s <cmd>` | 创建 + snap 模式 |
| `wt cd <branch>` | 切换到 worktree |
| `wt ls` | 列出 worktree |
| `wt ls -l` | 显示每个 worktree 的完整路径 |
| `wt main` | 返回主仓库 |
| `wt mv <old> <new>` | 重命名 worktree（`.` 表示当前） |
| `wt rm <branch>` | 删除 worktree（`.` 表示当前） |
| `wt rm -f <branch>` | 强制删除（含未提交更改） |
| `wt clean` | 清理与 trunk 无差异的 worktree |
| `wt clean --dry-run` | 预览将被清理的 worktree（不实际删除） |

### 工作流

| 命令 | 描述 |
|------|------|
| `wt merge` | 合并到 base 分支（fallback trunk，默认 squash） |
| `wt merge -s <strategy>` | 指定合并策略（squash/merge） |
| `wt merge --into <branch>` | 合并到指定分支（覆盖 base） |
| `wt merge -d` | 合并后删除 worktree（默认保留） |
| `wt merge -H` | 跳过 pre-merge hooks |
| `wt sync` | 从 base 分支同步更新（fallback trunk，默认 rebase） |
| `wt sync -s <strategy>` | 指定同步策略（rebase/merge） |
| `wt sync --from <branch>` | 从指定分支同步（覆盖 base） |
| `wt sync --continue` | 解决冲突后继续 |
| `wt sync --abort` | 放弃同步 |

### 信息

| 命令 | 描述 |
|------|------|
| `wt status` | 显示当前 worktree 信息 |
| `wt update` | 更新到最新版本 |

### 配置

| 命令 | 描述 |
|------|------|
| `wt setup` | 安装 shell 集成（自动检测） |
| `wt setup --shell zsh` | 为指定 shell 安装 |
| `wt init` | 初始化项目配置 |
| `wt init --trunk <branch>` | 初始化并指定 trunk 分支 |
| `wt init --merge-strategy <strategy>` | 设置默认合并策略（squash/merge） |
| `wt init --copy-files <pattern>` | 指定要复制到新 worktree 的文件（可重复） |

## 配置文件

### 全局配置 `~/.agent-worktree/config.toml`

```toml
[general]
merge_strategy = "squash"  # squash | merge
trunk = "main"  # trunk 分支（省略则自动检测）
copy_files = [".env", ".env.*"]  # gitignore 风格的文件模式

[hooks]
post_create = ["pnpm install"]
pre_merge = ["pnpm test", "pnpm lint"]
post_merge = []
```

### 项目配置 `.agent-worktree.toml`

```toml
[general]
copy_files = ["*.secret.*"]
# ...
```

## 存储结构

```
~/.agent-worktree/
├── config.toml                    # 全局配置
└── workspaces/
    └── {project}/
        ├── swift-fox.toml         # worktree 元数据
        ├── swift-fox/             # worktree 目录
        └── ...
```

## 许可证

MIT
