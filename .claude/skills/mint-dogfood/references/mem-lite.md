# mint × mem-lite 关联机制

mint 管**可执行 issue**（生命周期），mem-lite 管**事实/教训记忆**。双记忆模式：适合固化到项目的记忆写 `notes/`，其他写 mem-lite；mem-lite 允许与 notes 重复、有大交集。

**交叉通过 `refs` 互引**（见 `notes/DDD.md`「与 mem-lite 的分工」），不强耦合、不自动摘要。

## 双向引用格式

| 方向 | 载体 | 格式 | 示例 |
|------|------|------|------|
| mint → mem-lite | issue 的 `--body` | `memory#<mem-lite id>` | `--body "参考 memory#123"` |
| mem-lite → mint | observation 文本 | `issue#<mint id>；读取: <MINT> show <id> --json` | `...（issue#3；读取: ./target/release/mint show 3 --json）` |

## mem-lite 保存时携带 mint 关联

当某条 observation 对应一个 mint issue 时，narrative 中追加 mint issue id 与读取命令：

```bash
claude-mem-lite save "<内容>（关联 issue#<id>；读取: <MINT> show <id> --json）" \
  --project mint --type <decision|bugfix|discovery> --importance <1-3>
```

- `<MINT>` 是探测回退链解析出的调用前缀（`SKILL.md` 步骤 1：which mint → `./target/release/mint` → `./target/debug/mint` → `cargo run --`）。
- 读取命令写完整可执行形式，使任何会话可直接运行取回 mint JSON 内容。

## 从 mem-lite 读取 mint 内容

1. `mem_search <query>` 命中 observation，读到其中的 `issue#<N>`。
2. 运行 `<MINT> show <N> --json`，取回该 issue 完整 JSON（字段：`id/title/body/kind/status/project/test_cmd/dropped_reason/tags/created_at/updated_at`）。
3. 需要历史/全量时 `<MINT> list --all --json`。

## mem-lite 不存在时（降级）

- 前置探测：`which claude-mem-lite`。失败 → **跳过 mem-lite 保存**，仅用 mint 记录；本 skill 其余功能不受影响。
- mint issue 里的 `memory#N` 引用此时无对应目标，不写。
- mem-lite 是**增强项**，非依赖：缺失时 mint-dogfood 的登记/查询/状态机全部照常。
