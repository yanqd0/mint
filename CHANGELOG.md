# Change Log

## 0.6.0

### Optimizations

Measured against the previous release (0.5.0), same project database, median of multiple runs:

| Metric | 0.5.0 | 0.6.0 | Change |
|---|---|---|---|
| Release binary size | 2,318,992 B | 1,659,424 B | **-28%** |
| Cold startup (first DB) | 22.02 ms | 7.17 ms | **-67%** |
| Warm startup | 19.38 ms | 4.70 ms | **-76%** |
| Search latency | 18.3–19.6 ms | 4.7–4.9 ms | **-75%** |

Key contributors: TUI feature-gating (-350KB), SQLite compile-time trimming (-136KB + -100KB), opt-level z + dist LTO fat, WAL journal-mode skip on startup. Search improvement is dominated by the faster startup. Volume baseline documented (crate share + segment distribution) for future regression tracking.

### Optimization Evaluations

| Optimization | Verdict | Data |
|---|---|---|
| serde_json manual serialization | Rejected | 0.7% of binary, rewrite cost not justified |
| rusqlite cache feature removal | Rejected | +32B (LTO already removed dead code) |
| target-cpu=x86-64-v2 | Rejected | -0.18%, below 2% threshold |
| gc-sections / force-unwind-tables | Rejected | no gain on macOS/musl, rustc defaults suffice |
| build-std | Rejected | nightly toolchain complexity exceeds benefit |
| FTS5 sub-feature trimming | Rejected | flags absent in bundled SQLite 3.51.3 |
| Delivery compression | Confirmed optimal | default .tar.xz, 22% smaller than gzip |

### Features

- TUI list panels show parent resources: plans display milestone version; issues display version (blank when unattached).
- TUI search Esc clears the active filter and resets cursor/page, reverting to unfiltered results.
- List filtering expanded and combinable: plan/milestone filter by status and milestone ('' = unattached); issue filter by kind and plan; both support `--created-after`/`--updated-after` with date-prefix completion.
- Removed per-command `--tui` flag (TUI entry is the `mint tui` subcommand only).
- List panel column widths sized to content estimates (Flex::Legacy), freeing TITLE space on narrow screens.

### Bug Fixes

- PlanDetail `m` key now jumps to its milestone (missing branch in selected_milestone_id).
- Plans DONE/TOTAL column truncated totals; data and header now render fully.

### Others

- Added cargo-deny dependency audit to CI (CVE/license/duplicate-version gates).
- Replaced git subprocess calls with direct .git/ file parsing.
- npm OIDC trusted publishing (node22 + npm11, removed NODE_AUTH_TOKEN fallback).
- Updated skill docs with new list filter parameters (CN/EN synced).

## 0.5.0

### Features

- Agent ecosystem: Codex adapter (hook-based failure heuristic + AGENTS.md), OpenCode plugin (event-stream relay), skill multi-agent support (host-identification routing + `references/agent/`).
- Release pipeline: GitHub Actions gate (fmt/clippy/test on three platforms + 90% coverage ratchet) + cargo-dist (npm shell + GitHub Release, musl static) + crates.io/PyPI publishing (tag-activated, compatible with and without a `v` prefix).
- `mint export`: full JSON/TSV dump (issues with labels/links + plans + milestones + labels) for backup/migration; backup & migration guide `docs/BACKUP.md`.
- Release precheck script `scripts/precheck.sh` (version consistency / CHANGELOG / lint one-shot).
- Search enhancements: typed secondary filtering (exact ID pinned first + same-prefix follow, status/kind aliases), TUI match highlighting, unified TUI/CLI search semantics.
- TUI enhancements: list-panel titles show size (current page / total), labels colored by their recorded value (foreground auto-inferred for contrast), all labels shown in list.
- Label governance: removed version labels (expressed via milestone), module-label system (CI/MCP/TUI/cli/db/docs/plugin/search), full palette assignment.

### Docs

- README three-registry install details (crates.io/PyPI/npm).
- `docs/RELEASING.md` release operations guide.

## 0.4.0

### Features

- `mint tui` dashboard: auto-changing issue/plan/milestone panels + progress bars + status dots (● yellow/green-blink/green/white/red), TTY auto-refresh every second, non-TTY text snapshot; plan in execution auto-switches to plan panel, switches to its milestone on completion, then back to issues; Enter opens issue detail / milestone panel shows plan rows.
- `mint tui` UI rebuild: 6 pages (Issues/Plans/Milestones tabs + Issue/Plan/Milestone details), top tabs (1/2/3/Tab to switch), rounded panels, plan-detail kanban (6-state columns), plans grouped by milestone, auto-switch with idle constraint; UI polish (outer-frame project name, raised tab highlight, action hints, page total).
- `mint tui` auto-navigation: event-driven dual-queue pipeline (delayed merge + bounded queue + idle execution); new issue/plan/milestone jumps to list+detail (2s flash on change); issue status changes jump to its plan, plan end jumps to its milestone, 60s idle returns home.
- TUI lists: issues/plans/milestones switched to ratatui Table (width-aligned) + STATUS/P/Kind columns + milestones numeric columns (PLANS·ISSUES total(direct)) + TITLE top-aligned ellipsis + no default selection + dynamic page size by panel height.
- TUI detail pages: basic compact `key: value | ...` kv (whole-pair wrap, status colored, created/updated purple); plan/milestone body ≤10 lines ellipsized, issue body fills bottom; milestone detail plans/issues panels status dots (container/issue color) + direct-first all-issues display + aggregate progress panel; show detail basic auto-wraps over-width without truncation.
- TUI progress bars: 4-group aggregate (done/open/working/dropped) + eighth-block subpixel (dropped always visible) + group-percentage line + global palette alignment (open white/working yellow/done green/dropped red, partial cyan) + plans PROGRESS reuses plan-detail fill logic.
- TUI navigation: Ctrl+C quits (TuiKey keeps modifiers), Backspace history route chain (forward/back), milestone dual panels independently paged + cross-panel cursor routing, plan-detail Enter into issue / issue-detail p/m into plan/milestone.
- list `--tui` table browsing: TTY ratatui paged table (j/k or ↑/↓ row, PgUp/PgDn or h/l page, q/Esc quit), non-TTY falls back to single-page text; `--tui` conflicts with `--json`; columns aligned by Unicode display width.
- list default output switched to TSV: header row + tab-separated data rows (token-optimal); `--tsv` flag removed (TSV is the default), `--json`/`--tui` kept.
- `show` & details: show defaults to TSV (issue/plan/milestone, body last column escaped); issue detail basic dynamic columns + tags/test/body/links panels; `show --tui` reuses the dashboard detail page; `list --tui` unifies into the dashboard list page (Enter detail / Esc back).
- Plugin & CLI: root marketplace.json supports git-URL remote install; label list dropped `--tui`; project list TSV-ified.

### Bug Fixes

- Plugin loading: hooks.json top-level `hooks` wrapper key (fixes plugin load failure); hook timeout raised to 50/100 to tolerate both ms and second units.
- TUI display: long titles top-aligned ellipsis to prevent overflow/hard corner cuts (kanban column titles, panel_wrap/body titles, three-list TITLE, milestone-detail plan/issue rows); plans group-paging group title re-displayed across pages to avoid "missing plan" (plan loss); issue list title always `issues`; plans/milestones STATUS column colored by container status; progress percentage present group min 1% (aligned with progress-bar visibility).

### Others

- New deps: ratatui 0.30, crossterm 0.29, unicode-width.
- Shared code: pagination trio hoisted to `src/cli/list_common.rs`; TUI render layering under `src/tui/` (model pure state machine / draw / rows); dashboard recursive split (data/diff/draw/model/model_nav/model_view/run/types + pages/), model.rs split to ≤300 lines.
- Migration merged back to 001_init.sql (v1 baseline); notes gained status.md lifecycle & coloring authoritative doc; roadmap default-TSV strategy.
- Plugin specs: `title-templates/` (title semantics + ≤60-char cap) + `body-templates/` (T1-T16 scenario templates); SKILL.md slim sections + flow mini-index, en/cn synced.
- `state retest` command (test→dev fail-back, keeps `last_commit_id` marking the failure, requires `--test-cmd`); skill unified-testing mode (same-plan issues stay test → unified verify → unified close).
- Decision records: TUI choice (D25), default TSV (D26), mint tui dashboard (D27).

## 0.3.0

### Features

- Dedup: `add` auto-merges duplicate issues within a project (title normalization + fuzzy match, duplicates bump `hit_count`).
- Full-text search: `mint search <q>` (FTS5 trigram, CJK substring retrieval, project/label/status filters).
- Claude Code plugin adapter: bilingual skills `mint-faa` / `mint-faa-cn` + hooks (failure-signal injection, SessionStart pending-issues injection) + private marketplace.
- `mint edit <ID>`: update issue title/body (unspecified fields kept; title/body changes sync the search index).

### Bug Fixes

- Fixed plugin marketplace structure (unified private market, `claude plugin validate` passes).

### Others

- Decision records: dedup algorithm (D22), FTS decision (D23), multi-agent adaptation (D24).
- Skill body migration + command-reference sync; schema migration made incremental (merged back to baseline before release).

## 0.2.0

### Features

- Containers (roadmap/plan): two-level "containers" above issues; `mint roadmap` / `mint plan` subcommands (create/list/show + state ops).
  - schema v2: roadmaps/plans tables + lightweight migration framework; hierarchy `issue→plan(plan_id)`, `plan→roadmap(roadmap_id)`, roadmap direct issue mount (roadmap_direct_issues, either-or).
  - roadmap key fields `version`(UNIQUE) + `body`; plan `body`.
  - Container status 5-state derived: open/running/partial/dropped/done, cascade-synced on write (issue→plan→roadmap).
- git commit association: `state commit` (renamed/merged from `state stage`, dev→test) requires `--sha`, written to `issues.last_commit_id`; top-level commit subcommand removed.
- issue links: `mint link create/remove/list`, typed `related`/`solves`/`duplicates` many-to-many (schema v3, one-way store + reverse derivation), links embedded in show.
- Timezone display fix: stored UTC, displayed in local timezone (`datetime(col,'localtime')`).
- `--all/-a` short alias: unified across all list commands (default hides dropped/done, `-a` full).
- `mint delete` top-level command: dangerous ops consolidated ( `mint delete issue|plan|roadmap <id>`), delete SQL is a full transaction (unbind associations first, then delete), atomic with derived-status sync.

### Bug Fixes

- Fixed old v2 DB upgrades where container tables didn't update: 002 in-place DDL change skipped a migration; changed to 004 incremental rebuild (DROP+recreate, 0.2.0 unreleased so container tables empty — no data loss).
- Fixed display time not converted to local timezone.
- Fixed container-list count/plurality mis-suffixed on the title ("title s issues"); count plurality moved after issue.

### Others

- Migration merge: 4 migrations merged into 1 (001 final schema, user_version rebased to 1), upgrade-only UT cleaned up.
- Test system: UT heavily parameterized (rstest: full state-machine matrix / enum round-trips / format field combos), cargo-llvm-cov coverage measured (85%→91%), ST added coarse-grained migration and container derivation boundaries.
- SQL spec: split to src/db/CLAUDE.md (organization/simple spec/migration philosophy), sqruff_format Stop hook (formats only changed files), full layout overhaul (SELECT one column per line).
- issue label global rename: `tag`→`label` (tags/issue_tags→labels/issue_labels, `--tag`→`--label`, `mint tag`→`mint label`), distinguished from git tag / roadmap version semantics.
- mint-dogfood skill rebuilt as flow injection: descriptive args + flow references (bug/requirement/review/todo/planning/conditions/session) + new-session takeover mode.
- Dev spec: commit-self-consistency principle, RENAME foreign-key pitfall, INSERT OR IGNORE caveat, parameterized-first convention written into src/CLAUDE.md.
- Version bumped to 0.2.0-alpha.1.

## 0.1.0

### Features

- Core issue system: SQLite-backed global issue-tracking CLI with add/list/show and the full 6-state machine (open/planned/dev/test/done/dropped).
  - 4-table schema (projects/issues/tags/issue_tags), project auto-detection (explicit → git repo name → dirname → default).
  - tag `name:desc` syntax, free registration and issue association, `mint tag list` for agent semantics.
  - User-facing output all-English (i18n baseline); `--json` structured output.
- Dev-spec consolidation (dogfooding infrastructure): four-group `use` ordering spec, src/CLAUDE.md checklist, Stop hook auto-format, sqruff SQL check, SQL extracted to src/db/*.sql and parameterized, CLI-level end-to-end ST tests, project-level tester agent.
- mint-dogfood skill: early experimental adapter for Claude Code to proactively record/advance this project's issues (0.3.0 groundwork).

### Bug Fixes

- Fixed `drop --reason` silently discarded and `reset` not clearing test_cmd.
- Fixed first-run failure when the DB parent directory didn't exist.
- Fixed clippy-flagged DoubleEndedIterator usage.
- Fixed Stop hook depending on the working directory, and no degradation on cargo errors.
- Fixed reopen leaving residual `dropped_reason` (old-period field meaningless after reopen).
- Fixed production-code `expect` violations, project registration swallowing real errors, and close validation order masking invalid transitions.
- Fixed `--tag "a:"` producing a malformed tag name; added empty-value validation for title/`--project`.
- Concurrency robustness: cmd_add transactional atomic commit (BEGIN IMMEDIATE), idempotent project/tag registration, busy_timeout + WAL.

### Others

- Project init & build config (cargo skeleton, release optimization, .cargo/config.toml).
- Docs system (CLAUDE.md, src/CLAUDE.md, notes/ memory & planning, CONTRIBUTING, .vscode config).
- SQL extraction refactor + cmd_list parameterization (behavior preserved); use-statement grouping; state ops consolidated into `mint state <action>`; config subcommand removed, unified env-var prefix.
