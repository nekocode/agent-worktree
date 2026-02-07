# agent-worktree 架构设计文档

## 概述

agent-worktree 是一个 Git Worktree 工作流工具，为 AI coding agent 提供隔离的并行开发环境。

**核心价值**：
- 并行开发：同时运行多个 agent，互不干扰
- 环境隔离：每个功能独立工作目录
- 流程自动化：`-s/--snap` 模式实现"即用即删"的完整开发闭环

---

## 目录结构

```
~/.agent-worktree/
├── config.toml                    # 全局配置
└── workspaces/                    # 所有 worktree 存储位置
    └── {project}/                 # 按项目组织
        ├── swift-fox.status.toml  # worktree 的状态信息
        ├── swift-fox/             # 随机生成的分支名
        ├── fix-auth-bug.status.toml
        ├── fix-auth-bug/          # 用户指定的分支名
        ├── quiet-moon.status.toml
        └── quiet-moon/
            └── ...                # 项目文件

项目根目录/
└── .agent-worktree.toml           # 项目级配置（可选）
```

### 元数据格式

```toml
created_at = 2024-01-15T10:30:00Z
base_commit = "abc1234"
trunk = "main"
```

---

## 命令设计

### 1. Worktree 管理

```bash
wt new [branch]              # 创建 worktree 并进入
wt new [branch] --base <ref> # 基于指定 commit/分支创建（默认基于 trunk 最新）
wt new [branch] -s <cmd>     # 创建 + snap 模式
wt cd <branch>               # 切换到指定 worktree
wt ls                        # 列出 worktree
wt main                      # 回到主仓库
wt mv <old> <new>            # 重命名 worktree 分支（old 可用 . 表示当前）
wt rm <branch> [-f]          # 删除 worktree（branch 可用 . 表示当前）
wt clean                     # 清理所有与 trunk 无差异的 worktree
```

### 2. 工作流

```bash
wt merge [options]           # 合并当前 worktree 到主分支
    -s, --strategy <squash|merge|rebase>  # 合并策略，默认 squash
    --into <branch>          # 合并到指定分支（覆盖 trunk 设置）
    -k, --keep               # 合并后保留 worktree
    -H, --skip-hooks         # 跳过 pre-merge hook
    --continue               # 解决冲突后继续
    --abort                  # 放弃合并，恢复到冲突前状态

wt sync [options]            # 从 trunk 同步更新到当前 worktree
    -s, --strategy <rebase|merge>  # 同步策略，默认 rebase
    --continue               # 解决冲突后继续
    --abort                  # 放弃同步，恢复到冲突前状态
```

### 3. 维护

```bash
wt update                    # 更新到最新版本
```

### 4. 配置

```bash
wt setup                     # 安装 shell 集成（自动检测 shell）
wt setup --shell zsh         # 指定 shell
wt init                      # 在当前项目初始化配置
wt init --trunk <branch>     # 初始化并指定主干分支
```

---

## Shell 集成

`wt cd`、`wt main`、`wt new`、`wt rm`、`wt mv`、`wt merge`、`wt clean` 等命令需要改变 shell 工作目录，因此需要 shell wrapper。

运行 `wt setup` 自动安装（npm 安装时会自动执行），会在 shell 配置文件中添加 wrapper 函数。

**支持的 shell**：bash、zsh、fish、powershell

**配置文件位置**：
- bash: `~/.bashrc`
- zsh: `~/.zshrc`
- fish: `~/.config/fish/config.fish`
- powershell: `~/Documents/PowerShell/Microsoft.PowerShell_profile.ps1`

Wrapper 会检查 `wt` 命令是否存在，不存在时给出安装提示。

---

## 分支名生成

1. **用户指定**：`wt new fix-auth-bug` → 使用 `fix-auth-bug`
2. **自动生成**：`wt new` → 生成 `形容词-名词` 格式，如 `swift-fox`

词库内置约 100 个形容词 + 100 个名词。冲突时追加数字后缀（`swift-fox-2`）。

---

## Snap 模式

"即用即删"的完整流程：

```
创建 worktree → 进入目录 → 启动 agent → [开发] → agent 退出 → 检查更改 → 合并 → 清理
```

```bash
wt new -s claude  # 简单命令，随机分支名
wt new -s "aider --model sonnet"  # 带参数的命令需要引号
wt new fix-bug -s cursor  # 指定分支名
```

### Agent 退出处理

**正常退出**，检查 git 状态：

| 状态 | 行为 |
|------|------|
| 无改动（uncommitted=❌, commits=❌） | 直接清理 worktree |
| 只有 commits（uncommitted=❌, commits=✅） | prompt: [m] merge / [q] exit |
| 有未提交改动（uncommitted=✅） | prompt: [r] reopen / [q] exit |

**有 commits 时** prompt：
```
[m] Merge into trunk
[q] Exit snap mode
```

**有未提交改动时** prompt：
```
[r] Reopen agent (let agent commit)
[q] Exit snap mode
```

选择 `[q]` 退出时：
- 保留在当前 worktree（不 cd 到 main）
- worktree 完整保留，后续可手动处理：
```bash
git add . && git commit -m 'message'
wt merge          # merge 并清理
```

**异常退出**（crash / Ctrl+C），worktree 保留为普通 worktree

---

## Merge 冲突处理

### 状态持久化

merge 遇到冲突时，通过 `.git/WT_MERGE_BRANCH` 文件记录正在合并的分支名。
模仿 git 自身的 `MERGE_HEAD` 模式 — 简单文本文件，冲突时写入，完成后删除。

```
冲突发生 → 写入 WT_MERGE_BRANCH → 用户解决冲突 → --continue/--abort → 清除 WT_MERGE_BRANCH
```

### 冲突工作流

所有 merge 操作在 main repo 目录执行（非 worktree 目录）：

1. `wt merge` 检测到冲突 → 保存状态，提示用户解决
2. 用户在 main repo 解决冲突 → `git add <files>`
3. `wt merge --continue` → 提交、运行 post-merge hooks、清理 worktree
4. 或 `wt merge --abort` → 还原状态、清理 WT_MERGE_BRANCH

### 安全检查

执行 merge 前检查 main repo 是否有未完成的 merge/rebase/uncommitted changes，防止并发 merge 冲突。

### 统一 merge 入口

`merge::execute_merge()` 是唯一的 merge 执行函数，`snap_continue` 和 `wt merge` 共用，消除逻辑重复。

---

## Git 错误处理

`git/mod.rs` 中的 `extract_error()` 统一从命令输出提取错误信息：
- 优先使用 stderr（git 的常规错误输出）
- stderr 为空时 fallback 到 stdout（merge 冲突信息走 stdout）

适用于 `merge`、`commit`、`merge_continue` 等冲突相关命令。

---

## 配置文件

### 全局配置 `~/.agent-worktree/config.toml`

```toml
[general]
merge_strategy = "rebase"               # squash | merge | rebase
# 从主仓库复制到新 worktree 的文件（通常是被 gitignore 但开发必需的），支持 glob
copy_files = ["*.secret.*"]

[hooks]
post_create = []
pre_merge = []
post_merge = []
```

### 项目配置 `.agent-worktree.toml`

```toml
[general]
trunk = "main"                    # 主干分支，默认自动检测
copy_files = [".env", ".env.*"]

[hooks]
post_create = ["pnpm install"]
pre_merge = ["pnpm test", "pnpm lint"]
```

---

## 实现建议

### 技术选型

推荐 **Rust**：单二进制、无运行时依赖、跨平台、快速启动

### 模块划分

```
agent-worktree/
├── Cargo.toml           # 依赖：clap, serde, toml, directories, chrono, thiserror, rand, dialoguer, glob
├── npm/                 # npm 分发包
│   ├── agent-worktree/  # 主包（JS wrapper）
│   │   ├── package.json
│   │   ├── install.js   # postinstall：验证平台包 + 安装 shell wrapper
│   │   └── bin/wt.js    # CLI 入口，根据平台找到对应二进制执行
│   ├── agent-worktree-darwin-arm64/  # macOS ARM64 二进制
│   ├── agent-worktree-darwin-x64/    # macOS x64 二进制
│   ├── agent-worktree-linux-x64/     # Linux x64 二进制
│   └── agent-worktree-win32-x64/     # Windows x64 二进制
├── scripts/             # 构建与发布脚本
│   ├── build-npm.sh     # 编译二进制并复制到 npm 包
│   └── publish-npm.sh   # 同步版本号并发布到 npm
├── src/
│   ├── main.rs          # 入口，解析 CLI 并分发
│   ├── lib.rs           # 模块导出
│   ├── cli/
│   │   ├── mod.rs       # Cli struct + Command enum
│   │   └── commands/
│   │       ├── mod.rs   # 命令模块导出
│   │       ├── new.rs   # wt new [branch] [--base] [-s]
│   │       ├── ls.rs    # wt ls [-l] (status/commits/diff/path)
│   │       ├── cd.rs    # wt cd <branch>
│   │       ├── main.rs  # wt main
│   │       ├── rm.rs    # wt rm <branch> [--force]
│   │       ├── clean.rs # wt clean
│   │       ├── merge.rs # wt merge [-s] [--into] [-k] [--continue] [--abort]
│   │       ├── sync.rs  # wt sync [--strategy]
│   │       ├── move.rs  # wt mv <old> <new>
│   │       ├── setup.rs  # wt setup [--shell]
│   │       ├── init.rs   # wt init [--trunk]
│   │       ├── update.rs # wt update
│   │       └── snap_continue.rs  # snap 模式 agent 退出后的处理逻辑
│   ├── config/
│   │   └── mod.rs       # GlobalConfig + ProjectConfig + Config (merged)
│   ├── git/
│   │   └── mod.rs       # 调用 git CLI：worktree/branch/merge/rebase/reset/diff-stat
│   ├── meta/
│   │   └── mod.rs       # WorktreeMeta (.status.toml 读写)
│   ├── process/
│   │   └── mod.rs       # run_interactive, run_hook, run_hooks
│   ├── shell/
│   │   └── mod.rs       # Shell enum + wrapper 脚本生成 + install
│   ├── prompt/
│   │   └── mod.rs       # confirm, snap_exit_prompt, snap_merge_prompt
│   ├── update/
│   │   └── mod.rs       # 版本检查与自动更新 (npm registry)
│   └── util/
│       ├── mod.rs
│       └── branch_name.rs  # generate_branch_name, generate_unique_branch_name
```