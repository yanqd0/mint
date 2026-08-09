# Leftover / TODO / observation handling flow (flow-todo)

Trigger: user mentions leftovers / TODOs / improvements / observations / tech debt.

## Steps

1. **Record**: run `list --json` to check for duplicates → `add "<title>" --body "<description + source>"` (problem=problem, improvement=requirement).
2. **Mount (optional)**: only mount if there's a clear target version/plan (`plan issue` / `milestone issue`); otherwise don't mount.
3. **Schedule**: `state plan` to mark as scheduled, leave for later development; no forced advancement.
