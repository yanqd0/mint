# mint × mem-lite linkage mechanism

mint manages **actionable issues** (lifecycle), mem-lite manages **facts/lessons** (memories). Dual-memory model: items worth solidifying in the project go in `notes/`, others go to mem-lite; mem-lite allows overlap with notes and broad intersections.

**Cross-reference via `refs`** (see `notes/DDD.md` "division of labor with mem-lite"), loosely coupled, no auto-summary.

## Bidirectional reference format

| Direction | Carrier | Format | Example |
|-----------|---------|--------|---------|
| mint → mem-lite | issue `--body` | `memory#<mem-lite id>` | `--body "see memory#123"` |
| mem-lite → mint | observation text | `issue#<mint id>; read: mint show <id> --json` | `...（issue#3; read: mint show 3 --json）` |

## Saving mem-lite with mint linkage

When an observation corresponds to a mint issue, append the mint issue id and read command in the narrative:

```bash
claude-mem-lite save "<content> (linked issue#<id>; read: mint show <id> --json)" \
  --project mint --type <decision|bugfix|discovery> --importance <1-3>
```

## Reading mint content from mem-lite

1. `mem_search <query>` finds an observation containing `issue#<N>`.
2. Run `mint show <N> --json` to retrieve the full issue JSON.
3. For history / full scope: `mint list --all --json`.

## When mem-lite is absent (fallback)

- Probe: `which claude-mem-lite`. On failure → **skip mem-lite save**, use mint only; other skill functions unaffected.
- mint issue `memory#N` references have no corresponding target — don't write them.
- mem-lite is an **enhancement**, not a dependency: recording/querying/state-machine all work normally without it.
