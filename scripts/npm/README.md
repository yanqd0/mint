# scripts/npm

Tooling for the **npm installer** shipped as the `mint-faa` package (#504, #514,
#520).

The npm package is not maintained in this repository: cargo-dist renders it at
release time (`dist build --artifacts=global` → `mint-faa-<version>-npm-package.tar.gz`),
and its launcher (`binary-install.js`) deletes and re-creates the install
directory without any coordination, so two concurrent first runs destroy each
other's work. The release workflow therefore patches the generated launchers
before publishing; the patch and its regression tests live here.

## Files

| File | Purpose |
|---|---|
| `patch-installer.mjs` | CLI: applies the anchor patches to an unpacked npm package (or a single generated file). `--check` asserts an already-patched package. |
| `installer-patches.mjs` | Anchor-based rewriters for the generated launchers (`binary-install.js`, `binary.js`) plus the lock and download policy constants. |
| `patch.test.mjs` | Unit tests: anchors, idempotency, syntax, CLI behaviour. |
| `race.test.mjs` | Behaviour tests over loopback HTTP: an in-flight install must not touch the existing directory, four concurrent first installs must all succeed and clean up, staging must stay beside the install directory, a cross-device commit must fall back to a copy, a failed install must release the lock, and a lock owned by a dead process must be stolen immediately. |
| `sources.test.mjs` | Behaviour tests for the download policy: retry the failing source, fall through to the next source, do not retry a 4xx, and report one diagnostic line per source when everything failed. |
| `fixtures/binary-install.mint-faa-0.8.0.js` | Byte-exact copy of the cargo-dist 0.32.0-generated `binary-install.js` from the published `mint-faa@0.8.0` package (sha256 `f9cd1e11d9fdbbcaec5cf0a52145a0495d109967ac60363ced9e2bb6f28472dd`). |
| `fixtures/binary.mint-faa-0.8.1.js` | Byte-exact copy of the cargo-dist 0.32.0-generated `binary.js` from the published `mint-faa@0.8.1` package (sha256 `4355398e7705fdf14d3c7ece43ec1f2fee2010eecc590cc9a311b44d8258544b`). |

The fixtures are the anchor contract. When cargo-dist changes a generated file,
`patch-installer.mjs` fails with the list of missing anchors — update the anchors
against the new file and refresh the fixture in the same commit.

## What the patch changes

- **Install lock (#504)**: `Package.install()` serializes installs of one package
  directory through `<installDirectory>.lock` (`mkdir` is the atomic acquire),
  re-checks `exists()` after acquiring, and releases the lock on **every** settle
  path — the failure path rejects instead of calling `process.exit()` inside the
  promise chain, because that would skip the release (#514).
- **Lock ownership (#514)**: the lock directory carries `owner.json`
  (`pid`/`startedAt`/`host`). A lock whose owner process is gone is stolen
  immediately; a lock without an identifiable owner is only stolen once it is
  older than the stale threshold (10 min), which stays below the wait timeout
  (15 min) so the steal branch is always reachable.
- **Same-device staging (#514)**: the archive is unpacked into
  `<installDirectory>.staging-XXXXXX` — a sibling, so the commit rename cannot
  fail with `EXDEV` — and published with an atomic rename. Stale `.staging-*`
  siblings left by a crashed installer are removed once the lock is held.
- **Cross-device commit (#514)**: if the commit rename still fails with `EXDEV`
  (or a Windows `EPERM` volume edge), the staged tree is copied instead, so the
  install still completes. A failed install never deletes a working install.
- **Download sources (#520)**: `binary.js` hands the installer *every* entry of
  `artifactDownloadUrls` (upstream only ever used `[0]`, see its `FIXME`), and
  `binary-install.js` walks them in order with a per-source attempt budget.
- **Download policy (#520)**: the official source gets one attempt plus one
  retry; if both fail the next source (the mirror) gets one attempt plus one
  retry — four attempts in total with one mirror. Network-level failures (socket
  hang up, DNS, TLS, stalled body, 5xx, 429) are retried with a 1 s backoff;
  other 4xx switch source immediately. An idle download is aborted after 30 s
  (socket timeout) and a proxy CONNECT after 15 s, so a stalled transfer cannot
  hang an install forever.
- **Diagnostics (#520)**: when every source fails, the error carries one line per
  source with its last failure and attempt count, plus a hint about
  `HTTPS_PROXY`/mirrors — instead of the previous bare `socket hang up`.

## Usage

```bash
# local verification (sandboxed hosts need TMPDIR inside the workspace)
TMPDIR=$PWD/.tmp-test node --test scripts/npm/*.test.mjs

# apply inside a release job
node scripts/npm/patch-installer.mjs <unpacked-npm-package-dir>
node scripts/npm/patch-installer.mjs <unpacked-npm-package-dir> --check
```

The published package differs from the release asset by this patch (the GitHub
Packages publish already rewrites `package.json`), so the `.sha256` published for
the dist artifact stays valid for the artifact itself.
