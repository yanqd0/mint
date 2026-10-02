# scripts/npm

Tooling for the **npm installer** shipped as the `mint-faa` package (#504).

The npm package is not maintained in this repository: cargo-dist renders it at
release time (`dist build --artifacts=global` → `mint-faa-<version>-npm-package.tar.gz`),
and its launcher (`binary-install.js`) deletes and re-creates the install
directory without any coordination, so two concurrent first runs destroy each
other's work. The release workflow therefore patches that generated file before
publishing; the patch and its regression tests live here.

## Files

| File | Purpose |
|---|---|
| `patch-installer.mjs` | Anchor-based rewriter: adds an install lock (lock directory + double-check), unpacks into a private staging directory, then commits with an atomic `rename`. `--check` asserts an already-patched file. |
| `patch.test.mjs` | Unit tests: anchors, idempotency, syntax, CLI behaviour. |
| `race.test.mjs` | Behaviour tests against the patched launcher over loopback HTTP: an in-flight install must not touch the existing directory, and four concurrent first installs must all succeed and clean up. |
| `fixtures/binary-install.mint-faa-0.8.0.js` | Byte-exact copy of the cargo-dist 0.32.0-generated `binary-install.js` from the published `mint-faa@0.8.0` package (sha256 `f9cd1e11d9fdbbcaec5cf0a52145a0495d109967ac60363ced9e2bb6f28472dd`). |

The fixture is the anchor contract. When cargo-dist changes the generated
launcher, `patch-installer.mjs` fails with the list of missing anchors — update
the anchors against the new file and refresh the fixture in the same commit.

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
