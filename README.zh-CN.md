# agent-worktree

为 AI 编程 agent 设计的 Git worktree 工作流工具。提供隔离的并行开发环境。

[English](README.md)

## 为什么需要

AI 编程 agent 在隔离环境中工作效果最佳：

- **并行执行**：同时运行多个 agent，互不干扰
- **环境隔离**：每个功能独立工作目录
- **Snap 模式**："即用即删"工作流 — 创建 worktree、运行 agent、合并、清理

## 安装

```bash
npm install -g agent-worktree
```

Shell 集成会自动安装。手动重新安装：

```bash
wt setup
```

支持的 shell：bash、zsh、fish、PowerShell

## 快速开始

```bash
# 创建 worktree 并进入
wt new feature-x

# 列出所有 worktree
wt ls

# 切换到另一个 worktree
wt cd feature-y

# 返回主仓库
wt main

# 合并并清理
wt merge
```

## Snap 模式

AI agent 工作流一行搞定：

```bash
wt new --snap claude           # 随机分支名
wt new fix-bug --snap cursor   # 指定分支名
wt new --snap "aider --model sonnet"  # 带参数的命令
```

流程：创建 worktree → 进入 → 运行 agent → [开发] → agent 退出 → 检查更改 → 合并 → 清理

Agent 正常退出且有未提交更改时：
```
[c] 运行 git commit，然后合并
[r] 重新打开 agent 继续工作
[x] 放弃更改并退出
```

## 命令

### Worktree 管理

| 命令 | 描述 |
|------|------|
| `wt new [branch]` | 创建 worktree（省略则随机命名） |
| `wt new [branch] --base <ref>` | 基于指定 commit/分支创建 |
| `wt new [branch] --snap <cmd>` | 创建 + snap 模式 |
| `wt cd <branch>` | 切换到 worktree |
| `wt ls` | 列出 worktree 及状态 |
| `wt main` | 返回主仓库 |
| `wt mv <old> <new>` | 重命名 worktree（`.` 表示当前） |
| `wt rm <branch>` | 删除 worktree（`.` 表示当前） |
| `wt rm <branch> --force` | 强制删除（含未提交更改） |
| `wt clean` | 清理与 trunk 无差异的 worktree |

### 工作流

| 命令 | 描述 |
|------|------|
| `wt merge` | 合并当前 worktree 到 trunk |
| `wt merge -s <strategy>` | 指定合并策略（squash/merge/rebase） |
| `wt merge --into <branch>` | 合并到指定分支 |
| `wt merge --no-delete` | 合并后保留 worktree |
| `wt merge --continue` | 解决冲突后继续 |
| `wt merge --abort` | 放弃合并 |
| `wt sync` | 从 trunk 同步更新（rebase） |
| `wt sync -s merge` | 使用 merge 策略同步 |
| `wt sync --continue` | 解决冲突后继续 |
| `wt sync --abort` | 放弃同步 |

### 配置

| 命令 | 描述 |
|------|------|
| `wt setup` | 安装 shell 集成（自动检测） |
| `wt setup --shell zsh` | 为指定 shell 安装 |
| `wt init` | 初始化项目配置 |
| `wt init --trunk <branch>` | 初始化并指定 trunk 分支 |

## 配置文件

### 全局配置 `~/.agent-worktree/config.toml`

```toml
[general]
merge_strategy = "squash"  # squash | merge | rebase
copy_files = ["*.secret.*"]  # 复制到新 worktree 的文件

[hooks]
post_create = []
pre_merge = []
post_merge = []
```

### 项目配置 `.agent-worktree.toml`

```toml
[general]
trunk = "main"  # trunk 分支（省略则自动检测）
copy_files = [".env", ".env.*"]

[hooks]
post_create = ["pnpm install"]
pre_merge = ["pnpm test", "pnpm lint"]
```

## 存储结构

```
~/.agent-worktree/
├── config.toml                    # 全局配置
└── workspaces/
    └── {project}/
        ├── swift-fox.status.toml  # worktree 元数据
        ├── swift-fox/             # worktree 目录
        └── ...
```

## 许可证

MIT
