# Releasing mint

mint ships to **three registries** from a single git tag: crates.io (source), PyPI (maturin wheel), npm (cargo-dist installer). All three are **tag-activated**: `git tag X.Y.Z` (or `vX.Y.Z`) + `git push origin X.Y.Z` triggers the release workflows. Regular pushes to `master` only run the CI gate (`.github/workflows/ci.yml`).

Pre-release versions (`0.5.0-alpha.1` style) are **skipped** by crates.io/PyPI (the `gate` job sets `is_stable=false`) — only npm/GitHub Release gets a prerelease. Stable releases (`v1.0.0` and up) publish to all three.

## 1. Configure secrets (one-time)

### GitHub

Repo → **Settings → Secrets and variables → Actions → New repository secret**:

| Secret | Source | Purpose |
|---|---|---|
| `CODECOV_TOKEN` | [codecov.io](https://codecov.io) → login → Add repository → copy token | coverage upload + badge |
| `NPM_TOKEN` | [npmjs.com](https://npmjs.com) → Access Tokens → Generate → *granular*, packages read/write | `npm publish` (cargo-dist) |

### crates.io

1. Terminal: `cargo login` → open the URL → paste the API token (generates `~/.cargo/credentials`).
2. Create a second token for CI: crates.io → **Account settings → API Tokens → New token**.
3. GitHub → **Settings → Environments → New environment `crates-io`**:
   - Add `CARGO_REGISTRY_TOKEN` secret (the token from step 2).
   - Add yourself as **Required reviewers** (manual approval gate before publish).

### PyPI

Recommended: **trusted publishing (OIDC, no token)**.

1. PyPI → **Account settings → Publishing → Add a new pending publisher**:
   - Owner: `yanqd0`, Repository: `yanqd0/mint`, Workflow: `publish-pypi.yml`, Environment: `pypi`.
2. GitHub → **Settings → Environments → New environment `pypi`** (matching the name above).

> Alternative: set `PYPI_API_TOKEN` in the `pypi` environment and switch the publish step to `maturin publish`.

## 2. Release flow (per version)

```sh
# 1. Bump version (Cargo.toml is the single source of truth)
#    e.g. edit version = "1.0.0"  (also update plugin.json/marketplace.json for formal releases)

# 2. Commit + tag + push (you, manually)
git add Cargo.toml Cargo.lock claude-plugin/ && git commit -m "chore(release): 1.0.0"
git tag 1.0.0                # 不带 v 前缀（v1.0.0 亦可，两种均触发）
git push origin 1.0.0        # ← the only remote gesture; triggers all release workflows
```

3. Watch GitHub Actions:
   - `release.yml` (cargo-dist) → GitHub Release + npm publish
   - `publish-crates-io.yml` → approve the `crates-io` environment
   - `publish-pypi.yml` → OIDC publish (no approval if trusted publishing)

## 3. Verify

```sh
cargo install mint-faa && mint --version
pip install mint-faa && mint --version
npm install -g mint-faa && mint --version
```

## Notes

- **musl**: Linux release binaries are built against `x86_64-unknown-linux-musl` (static linking, single-file, no glibc dependency). glibc (`clang`/`mold`) is local-development only — see `.cargo/config.local.toml` (git-ignored).
- **`release.yml` is generated** by `cargo-dist` (`dist-workspace.toml` is the config source). Never hand-edit it — run `dist generate` instead.
- Version must match between the tag and Cargo.toml; `my-git-tag` validates this.
