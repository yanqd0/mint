#!/usr/bin/env node
// Rewrite cargo-dist's generated npm launcher (`binary-install.js`) so that a
// concurrent first install cannot destroy another process's install (#504).
//
// Why a patch instead of a source file: `binary-install.js` is not in this
// repository. cargo-dist renders it at release time into
// `mint-faa-<version>-npm-package.tar.gz`, and cargo-dist exposes no template
// override for it, so the release workflow runs this script on the unpacked
// package before `npm pack` / `npm publish` (see `.github/workflows/release.yml`).
//
// The rewrite is anchor-based on purpose: if a cargo-dist upgrade changes the
// generated file, the anchors stop matching and the release fails loudly instead
// of silently shipping an unpatched installer.
//
// Usage:
//   node scripts/npm/patch-installer.mjs <package-dir|binary-install.js>
//   node scripts/npm/patch-installer.mjs <package-dir|binary-install.js> --check

import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Script } from "node:vm";

export const MARKER = "// [mint #504] atomic install patch";
export const INSTALLER_FILENAME = "binary-install.js";

const FS_ANCHOR = `const {
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtemp,
  rmSync,
} = require("fs");`;

const FS_REPLACEMENT = `const {
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtemp,
  mkdtempSync,
  renameSync,
  rmSync,
  statSync,
} = require("fs");`;

const CLASS_ANCHOR = "class Package {";

const HELPERS = `// [mint #504] Install lock + staged commit helpers.
//
// The binary lives in a directory shared by every mint invocation of the same
// package ("node_modules/.bin_real"). The upstream installer deleted that
// directory and unpacked into it without any coordination, so two concurrent
// first runs (lazy \`run()\` or \`postinstall\`) destroyed each other's work.
// Installs are now serialized by a lock directory and only become visible
// through an atomic rename.
const INSTALL_LOCK_RETRY_MS = 250;
const INSTALL_LOCK_STALE_MS = 20 * 60 * 1000;
const INSTALL_LOCK_TIMEOUT_MS = 15 * 60 * 1000;
const INSTALL_LOCK_WARN_MS = 2000;

function acquireInstallLock(lockDir) {
  const startedAt = Date.now();
  const deadline = startedAt + INSTALL_LOCK_TIMEOUT_MS;
  return new Promise((resolve, reject) => {
    let warned = false;
    const attempt = () => {
      try {
        // mkdir(2) is atomic: whoever creates the directory owns the install.
        mkdirSync(lockDir);
        resolve();
        return;
      } catch (e) {
        if (e.code !== "EEXIST") {
          reject(
            new Error(\`Unable to create install lock \${lockDir}: \${e.message}\`),
          );
          return;
        }
      }
      try {
        const age = Date.now() - statSync(lockDir).mtimeMs;
        if (age > INSTALL_LOCK_STALE_MS) {
          rmSync(lockDir, { recursive: true, force: true });
          attempt();
          return;
        }
      } catch {
        // The lock disappeared between checks; retry below.
      }
      if (!warned && Date.now() - startedAt > INSTALL_LOCK_WARN_MS) {
        warned = true;
        console.error(
          \`Waiting for another mint-faa install to finish (\${lockDir})\`,
        );
      }
      if (Date.now() > deadline) {
        reject(new Error(\`Timed out waiting for install lock \${lockDir}\`));
        return;
      }
      setTimeout(attempt, INSTALL_LOCK_RETRY_MS);
    };
    attempt();
  });
}

function releaseInstallLock(lockDir) {
  try {
    rmSync(lockDir, { recursive: true, force: true });
  } catch {
    // best effort
  }
}

function withInstallLock(lockDir, fn) {
  return acquireInstallLock(lockDir).then(() => {
    let result;
    try {
      result = fn();
    } catch (e) {
      releaseInstallLock(lockDir);
      throw e;
    }
    return Promise.resolve(result).then(
      (value) => {
        releaseInstallLock(lockDir);
        return value;
      },
      (e) => {
        releaseInstallLock(lockDir);
        throw e;
      },
    );
  });
}

function commitStagedInstall(pkg, finalDir, stagingDir) {
  rmSync(finalDir, { recursive: true, force: true });
  renameSync(stagingDir, finalDir);
  pkg.installDirectory = finalDir;
}

function abortStagedInstall(pkg, finalDir, stagingDir) {
  pkg.installDirectory = finalDir;
  try {
    rmSync(stagingDir, { recursive: true, force: true });
  } catch {
    // best effort
  }
}

class Package {`;

const INSTALL_HEAD_ANCHOR = `  install(suppressLogs = false) {
    if (this.exists()) {
      if (!suppressLogs) {
        console.error(
          \`\${this.name} is already installed, skipping installation.\`,
        );
      }
      return Promise.resolve();
    }

    try {
      rmSync(this.installDirectory, { recursive: true, force: true });
    } catch {
      // ignore - directory may not exist
    }

    mkdirSync(this.installDirectory, { recursive: true });`;

const INSTALL_HEAD_REPLACEMENT = `  install(suppressLogs = false) {
    if (this.exists()) {
      if (!suppressLogs) {
        console.error(
          \`\${this.name} is already installed, skipping installation.\`,
        );
      }
      return Promise.resolve();
    }

    // [mint #504] Serialize same-directory installs, then re-check: the process
    // that held the lock may have finished while we were waiting.
    return withInstallLock(\`\${this.installDirectory}.lock\`, () => {
      if (this.exists()) {
        if (!suppressLogs) {
          console.error(
            \`\${this.name} is already installed, skipping installation.\`,
          );
        }
        return Promise.resolve();
      }
      return this.installStaged(suppressLogs);
    });
  }

  installStaged(suppressLogs) {
    // Unpack into a private staging directory and publish it with an atomic
    // rename, so readers never observe a half-written binary and a failed
    // install never destroys a working one.
    const finalDir = this.installDirectory;
    const stagingDir = mkdtempSync(join(tmpDir, "mint-faa-install-"));
    mkdirSync(stagingDir, { recursive: true });
    this.installDirectory = stagingDir;`;

const INSTALL_TAIL_ANCHOR = `      .then(() => {
        if (!suppressLogs) {
          console.error(\`\${this.name} has been installed!\`);
        }
      })
      .catch((e) => {
        error(\`Error fetching release: \${e.message}\`);
      });`;

const INSTALL_TAIL_REPLACEMENT = `      .then(() => {
        commitStagedInstall(this, finalDir, stagingDir);
        if (!suppressLogs) {
          console.error(\`\${this.name} has been installed!\`);
        }
      })
      .catch((e) => {
        abortStagedInstall(this, finalDir, stagingDir);
        error(\`Error fetching release: \${e.message}\`);
      });`;

const PATCHES = [
  { name: "fs imports", from: FS_ANCHOR, to: FS_REPLACEMENT },
  { name: "class Package", from: CLASS_ANCHOR, to: HELPERS },
  { name: "install() head", from: INSTALL_HEAD_ANCHOR, to: INSTALL_HEAD_REPLACEMENT },
  { name: "install() tail", from: INSTALL_TAIL_ANCHOR, to: INSTALL_TAIL_REPLACEMENT },
];

/** Outcome of applying {@link patchBinaryInstall}. */
export function patchBinaryInstall(source) {
  if (source.includes(MARKER)) {
    return { code: source, changed: false, missing: [], ambiguous: [] };
  }
  const missing = [];
  const ambiguous = [];
  for (const patch of PATCHES) {
    const occurrences = source.split(patch.from).length - 1;
    if (occurrences === 0) {
      missing.push(patch.name);
    } else if (occurrences > 1) {
      ambiguous.push(patch.name);
    }
  }
  if (missing.length > 0 || ambiguous.length > 0) {
    return { code: source, changed: false, missing, ambiguous };
  }
  let code = source;
  for (const patch of PATCHES) {
    code = code.replace(patch.from, patch.to);
  }
  code = `${MARKER} — applied by scripts/npm/patch-installer.mjs; do not edit by hand\n${code}`;
  return { code, changed: true, missing: [], ambiguous: [] };
}

/** Throws when `code` is not valid CommonJS JavaScript. */
export function assertSyntax(code, filename = INSTALLER_FILENAME) {
  new Script(code, { filename });
}

function describeFailure(missing, ambiguous) {
  const parts = [];
  if (missing.length > 0) {
    parts.push(`missing anchors: ${missing.join(", ")}`);
  }
  if (ambiguous.length > 0) {
    parts.push(`ambiguous anchors: ${ambiguous.join(", ")}`);
  }
  return parts.join("; ");
}

function resolveTarget(target) {
  if (existsSync(target) && statSync(target).isDirectory()) {
    return join(target, INSTALLER_FILENAME);
  }
  return target;
}

function main(argv) {
  const args = argv.slice(2);
  const check = args.includes("--check");
  const targets = args.filter((arg) => !arg.startsWith("-"));
  if (targets.length !== 1) {
    console.error(
      "usage: node scripts/npm/patch-installer.mjs <package-dir|binary-install.js> [--check]",
    );
    return 2;
  }
  const target = resolveTarget(targets[0]);
  if (!existsSync(target)) {
    console.error(`not found: ${target}`);
    return 1;
  }

  const source = readFileSync(target, "utf8");
  if (check) {
    if (source.includes(MARKER)) {
      console.log(`already patched: ${target}`);
      return 0;
    }
    const { missing, ambiguous } = patchBinaryInstall(source);
    console.error(`not patched: ${target} (${describeFailure(missing, ambiguous)})`);
    return 1;
  }

  const { code, changed, missing, ambiguous } = patchBinaryInstall(source);
  if (missing.length > 0 || ambiguous.length > 0) {
    console.error(`cannot patch ${target}: ${describeFailure(missing, ambiguous)}`);
    console.error("cargo-dist changed the generated installer; update the anchors.");
    return 1;
  }
  if (!changed) {
    console.log(`already patched: ${target}`);
    return 0;
  }
  assertSyntax(code, target);
  writeFileSync(target, code);
  console.log(`patched: ${target}`);
  return 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main(process.argv);
}
