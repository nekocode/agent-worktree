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
base_branch = "main"             # 创建时的源分支（merge/sync 默认目标）
```

> 旧版字段（`base_commit`/`trunk`/`snap_command`）已弃用。读取时若缺 `base_branch` 则回退到旧 `trunk` 字段；其他旧字段静默忽略。

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
    -s, --strategy <rebase|merge>  # 同步策略，默认 rebase（可被 sync_strategy 配置覆盖）
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
    --sync-strategy <rebase|merge>   # 默认同步策略
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

### 集成约束

- **Wrapper 必装才能 cd**：`wt cd` 检测无 `--path-file` 直接报错，提示 `wt setup`——不再静默 noop
- **`wt rm .` 防误操**：cwd 在被删 worktree 内且无 wrapper → 拒绝（避免 dangling cwd）
- **rc 文件 marker 严格配对**：`wt setup` 找到孤立 BEGIN/END 直接报错，不动 rc，避免截断
- **path_file 唯一**：bash/zsh wrapper 用 `mktemp` 而非 `$$`（subshell 中 `$$` 是父 PID，并发会撞）
- **agent 退出统一**：crash/SIGINT/非零状态都进 snap-continue
- **Windows update**：`wt update` 调用 npm，运行中的 `wt.exe` 被 OS 锁定 → 先关闭所有 wt 进程

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

merge 为原子操作——要么成功，要么 HEAD 回到原分支。不残留中间状态。

```
wt merge
  → 记录主 repo 当前分支为 original
  → checkout target（main repo）
  → dry-run（按真实策略：squash 用 --squash --no-commit，否则 --no-ff）
  → 有冲突？
      YES → 清理 + checkout original → 报错 "先 wt sync 解决冲突"
      NO  → 清理 + 执行真实 merge
              失败 → reset_merge + checkout original → 抛错
              成功 → 跑 post_merge hook → 可选删 worktree
```

### 冲突处理流程

用户需在 worktree 中先 sync 对齐目标分支，再执行 merge：

```bash
wt sync          # 在 worktree 中解决冲突
wt merge         # 无冲突，原子完成
```

### 安全检查与约束

- 主 repo 的未完成 merge / rebase / uncommitted changes → 拒绝
- worktree dirty → 拒绝（消息明示是 worktree 端脏）
- 主 repo dirty → 拒绝（消息明示是 main repo 端脏）
- `--into <branch>` 已被另一 worktree checkout → 拒绝（避免 git 报底层错）
- `MergeStrategy::Merge` already-up-to-date → 返回 "Nothing to merge" 不删 worktree
- 失败一律 rollback HEAD 到原分支 + reset_merge 清 squash 半成品

### merge 入口

- `merge::execute_merge()` 处理 squash/merge 策略，`snap_continue` 和 `wt merge` 共用
- `git::dry_run_merge(branch, squash)` 用于预检测冲突，按策略走 `--squash --no-commit` 或 `--no-ff --no-commit`

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
sync_strategy = "rebase"                # rebase（默认） | merge
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
- `merge_strategy` / `sync_strategy`：project 非空时**覆盖** global（`Option` 语义）
- `trunk`：仅 project 级别配置

### 项目配置 `.agent-worktree.toml`

```toml
[general]
trunk = "main"                    # 主干分支，默认自动检测
merge_strategy = "merge"          # 可选，覆盖全局策略
sync_strategy = "merge"           # 可选，覆盖全局同步策略
copy_files = [".env", ".env.*"]

[hooks]
post_create = ["pnpm install"]
pre_merge = ["pnpm test", "pnpm lint"]
```

### 配置约束与信任边界

- **路径解析**：项目配置从 `git rev-parse --git-common-dir` 上溯到主 repo 根读取——worktree/子目录任意位置行为一致
- **`copy_files` 路径沙箱**：拒绝 `/` 开头（绝对路径）和 `..` 段；不跟随符号链接
- **hooks 安全**：hooks 通过 `sh -c`（Windows `cmd /C`）执行，无沙箱无超时——按"committed shell script"信任处理，禁运行不信任 repo
- **hook CWD**：`pre_merge`/`post_merge` 一律 worktree 根；`post_create` 在新 worktree 内
- **trunk 检测**：`origin/HEAD` > `main` > `master` > 默认 `"main"`

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
│   │       │   ├── init.rs    # wt init [--trunk] [--merge-strategy] [--sync-strategy] [--copy-files]
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