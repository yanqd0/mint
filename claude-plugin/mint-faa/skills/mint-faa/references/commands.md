# mint commands (quick reference)

All commands support `--json` (agent-friendly output). Global `--db <PATH>` (or
`MINT_DB_PATH`) overrides the default DB. Project is auto-detected: `--project`
→ git repo name → dirname → `default`.

| Command | Purpose |
|---|---|
| `mint add <TITLE> [--body <BODY>] [--kind problem\|requirement] [--project <NAME>] [--label <name>...]` | Create issue. Dedupe built-in (same-project active-issue title fuzzy match). |
| `mint search <QUERY> [--project <NAME>] [--label <NAME>] [--status <S>]` | Full-text search (FTS5 trigram, query ≥3 chars). |
| `mint list [--all\|-a] [--status <S>] [--label <NAME>] [--project <NAME>]` | List issues. Active (open/planned/dev/test) by default; `--all` includes done/dropped. |
| `mint show <ID>` | Issue details (labels, links, commit, hit_count). |
| `mint edit <ID> [--title <T>] [--body <B>]` | Update title/body (COALESCE keeps unprovided fields; empty body clears; FTS sync). |
| `mint state plan\|start\|commit\|close\|reset\|drop\|reopen <ID> [--sha <SHA>] [--test-cmd <CMD>] [--reason <TEXT>]` | State transitions. `commit` requires `--sha`; `close` requires `--test-cmd`. |
| `mint label list [--all]` | List labels (with issue counts). |
| `mint roadmap create <TITLE> --version <V> \| list \| show <ID>` | Roadmap containers. |
| `mint plan create <TITLE> [--roadmap <ID>] \| list \| show <ID>` | Plan containers. |
| `mint link create <FROM> <TYPE> <TO>` | Issue links: related / solves / duplicates. |
| `mint delete issue\|plan\|roadmap <ID>` | DANGEROUS permanent delete; prefer `state drop`. |

Default DB: `$XDG_DATA_HOME/mint/mint.db` (macOS: `~/.local/share/mint/mint.db`).
