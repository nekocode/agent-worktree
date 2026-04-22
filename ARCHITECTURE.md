# agent-worktree 架构设计文档

## 概述

agent-worktree 是一个 Git Worktree 工作流工具，为 AI coding agent 提供隔离的并行开发环境。

**核心价值**：
- 并行开发：同时运行多个 agent，互不干扰
- 环境隔离：每个功能独立工作目录
- 流程自动化：`-s/--snap` 模式实现"即用即删"的完整开发闭环

---

## 目录结构

基础目录默认 `~/.agent-worktree`，可通过 `AGENT_WORKTREE_DIR` 环境变量覆盖（空串视同未设）。

```
$AGENT_WORKTREE_DIR/  (默认 ~/.agent-worktree/)
├── config.toml                    # 全局配置
└── workspaces/                    # 所有 worktree 存储位置
    └── {repo}-{hash}/             # 按项目组织（hash 基于仓库绝对路径，防止同名冲突）
        ├── swift-fox.toml         # worktree 元数据（旧版 .status.toml 仍兼容）
        ├── swift-fox/             # 随机生成的分支名
        ├── fix-auth-bug.toml
        ├── fix-auth-bug/          # 用户指定的分支名
        ├── quiet-moon.toml
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
base_branch = "feature-a"       # 可选，基底分支（merge/sync 的默认目标，--base 可自定义）
snap_command = "claude"          # 可选，snap 模式时记录启动命令
```

---

## 命令设计

### 1. Worktree 管理

```bash
wt new [branch]              # 创建 worktree 并进入（从当前分支 checkout，记录为 base branch）
wt new [branch] --base <br>  # 指定 base branch（覆盖当前分支，从该分支 checkout）
wt new [branch] -s <cmd>     # 创建 + snap 模式
wt cd [branch]               # 切换到指定 worktree（省略则回到主仓库）
wt ls                        # 列出 worktree（按创建时间降序）
wt status                    # 查看当前 worktree 详细信息
wt mv <old> <new>            # 重命名 worktree 分支（old 可用 . 表示当前）
wt rm <branch> [-f]          # 删除 worktree（branch 可用 . 表示当前）
wt clean [--dry-run]         # 清理所有与 target 无差异的 worktree（target = base_branch > trunk）
```

### 2. 工作流

```bash
wt merge [options]           # 合并当前 worktree（默认 merge 回 base branch，fallback trunk）
    -s, --strategy <squash|merge>  # 合并策略，默认 squash
    --into <branch>          # 合并到指定分支（覆盖 base branch / trunk，校验存在性）
    -d, --delete             # 合并后删除 worktree（默认保留）
    -H, --skip-hooks         # 跳过 pre-merge hook

wt sync [options]            # 从 base branch 同步更新到当前 worktree（fallback trunk）
    -s, --strategy <rebase|merge>  # 同步策略，默认 rebase
    --from <branch>          # 指定同步源分支（覆盖 base branch / trunk，校验存在性）
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
wt init [options]            # 在当前项目初始化配置
    --trunk <branch>         # 主干分支
    --merge-strategy <squash|merge>  # 默认合并策略
    --copy-files <pattern>   # 复制文件模式（可重复）
```

---

## Shell 集成

`wt cd`、`wt new`、`wt rm`、`wt mv`、`wt merge`、`wt clean` 等命令需要改变 shell 工作目录，因此需要 shell wrapper。

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

### 原子 merge（预检测模式）

merge 为原子操作——要么成功，要么什么都不做。不存在中间状态。

```
wt merge
  → checkout target（main repo）
  → dry-run: git merge --no-commit --no-ff <branch>
  → 有冲突？
      YES → git merge --abort → 报错 "先 wt sync 解决冲突"
      NO  → git merge --abort → 真正执行 merge（squash/merge 策略）
```

### 冲突处理流程

用户需在 worktree 中先 sync 对齐目标分支，再执行 merge：

```bash
wt sync          # 在 worktree 中解决冲突
wt merge         # 无冲突，原子完成
```

### 安全检查

执行 merge 前检查 main repo 是否有未完成的 merge/rebase/uncommitted changes，防止并发 merge 冲突。

### merge 入口

- `merge::execute_merge()` 处理 squash/merge 策略，`snap_continue` 和 `wt merge` 共用
- `git::dry_run_merge()` 用于预检测冲突

---

## Git 错误处理

`git/mod.rs` 中的 `extract_error()` 统一从命令输出提取错误信息：
- 优先使用 stderr（git 的常规错误输出）
- stderr 为空时 fallback 到 stdout（merge 冲突信息走 stdout）

适用于 `merge`、`commit`、`merge_continue` 等冲突相关命令。

---

## 配置文件

### 全局配置 `$AGENT_WORKTREE_DIR/config.toml`（默认 `~/.agent-worktree/config.toml`）

```toml
[general]
merge_strategy = "squash"               # squash（默认） | merge
# 从主仓库复制到新 worktree 的文件（通常是被 gitignore 但开发必需的），支持 glob
copy_files = ["*.secret.*"]

[hooks]
post_create = []
pre_merge = []
post_merge = []
```

### 配置合并规则

- `copy_files`：global + project **追加**合并
- `hooks`：project 非空时**完全替代** global（不追加）
- `merge_strategy`：project 非空时**覆盖** global（`Option` 语义）
- `trunk`：仅 project 级别配置

### 项目配置 `.agent-worktree.toml`

```toml
[general]
trunk = "main"                    # 主干分支，默认自动检测
merge_strategy = "merge"          # 可选，覆盖全局策略
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
├── Cargo.toml           # 依赖：clap, serde, toml, directories, chrono, thiserror, rand, dialoguer, ignore, dirs, ureq
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
│   │       ├── mod.rs         # 模块声明 + Args 重导出
│   │       ├── nav/           # 导航（改变 shell 工作目录）
│   │       │   └── cd.rs      # wt cd [branch]（省略=回主仓库）
│   │       ├── lifecycle/     # Worktree 生命周期
│   │       │   ├── new.rs     # wt new [branch] [--base] [-s]
│   │       │   ├── rm.rs      # wt rm <branch> [--force]
│   │       │   └── clean.rs   # wt clean [--dry-run]
│   │       ├── snap/          # Snap 模式完整工作流
│   │       │   └── resume.rs  # agent 退出后的处理逻辑
│   │       ├── sys/           # 系统级操作
│   │       │   ├── init.rs    # wt init [--trunk] [--merge-strategy] [--copy-files]
│   │       │   ├── setup.rs   # wt setup [--shell]
│   │       │   └── update.rs  # wt update
│   │       ├── merge.rs       # wt merge [-s] [--into] [-d] [--continue] [--abort]
│   │       ├── sync.rs        # wt sync [--strategy] [--from]
│   │       ├── ls.rs          # wt ls [-l]（按 created_at 排序）
│   │       ├── status.rs      # wt status（当前 worktree 详细信息）
│   │       └── move.rs        # wt mv <old> <new>
│   ├── config/
│   │   └── mod.rs       # GlobalConfig + ProjectConfig + Config (merged)
│   ├── git/
│   │   ├── mod.rs       # Error 类型 + run/run_extract 辅助 + pub use 重导出
│   │   ├── repo.rs      # 仓库信息查询：repo_root, repo_name, workspace_id, current_branch, detect_trunk, branch_exists
│   │   ├── worktree.rs  # Worktree CRUD + WorktreeInfo：create/remove/move/list/parse
│   │   ├── branch.rs    # 分支操作 + 状态检查：is_merged, delete/rename_branch, diff_shortstat, commit_count
│   │   ├── ops.rs       # Git 执行操作：merge, rebase, commit, checkout, fetch, abort/continue
│   │   └── tests.rs     # 单元测试
│   ├── meta/
│   │   └── mod.rs       # WorktreeMeta (.toml 读写 + .status.toml 兼容) + resolve_target_branch()
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
└── tests/
    ├── common/
    │   └── mod.rs          # 共享测试辅助：wt_binary, setup_git_repo, setup_worktree_test_env
    ├── cmd_help.rs         # help/version/strategy 显示测试
    ├── cmd_init.rs         # wt init 测试
    ├── cmd_new.rs          # wt new + worktree 生命周期测试
    ├── cmd_ls.rs           # wt ls 测试
    ├── cmd_cd.rs           # wt cd 测试
    ├── cmd_rm.rs           # wt rm 测试
    ├── cmd_mv.rs           # wt mv 测试
    ├── cmd_sync.rs         # wt sync 测试
    ├── cmd_merge.rs        # wt merge 测试
    ├── cmd_clean.rs        # wt clean 测试
    ├── cmd_status.rs       # wt status 测试
    ├── cmd_main.rs         # wt cd（无参数回主仓库）测试
    ├── cmd_snap.rs         # snap 模式测试
    └── cmd_misc.rs         # 错误处理 + 未知命令测试
```