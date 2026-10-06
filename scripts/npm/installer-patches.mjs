// Anchor-based rewriter for the npm launcher that cargo-dist renders at release
// time (`binary-install.js`, and `binary.js` from #520 on). The generated files
// are not in this repository, so the release workflow patches the unpacked npm
// package right before publishing; see scripts/npm/README.md.
//
// The rewrite is anchor-based on purpose: if a cargo-dist upgrade changes the
// generated file, the anchors stop matching and the release fails loudly instead
// of silently shipping an unpatched installer.
//
// Patches: #504 (install lock + staged commit), #514 (same-device staging, lock
// ownership, EXDEV fallback), #520 (download retry/timeout/source fallback).

import { Script } from "node:vm";

/** Marker written into every patched file (idempotency check). */
export const MARKER =
  "// [mint npm-installer] hardened by scripts/npm/patch-installer.mjs; do not edit by hand";

export const INSTALLER_FILENAME = "binary-install.js";

/** Lock policy, interpolated into the generated installer so tests can assert it. */
export const LOCK = {
  retryMs: 250,
  warnMs: 2000,
  /** Age fallback when the lock owner cannot be identified; must stay below timeoutMs. */
  staleMs: 10 * 60 * 1000,
  /** Wait deadline for a lock held by a live owner. */
  timeoutMs: 15 * 60 * 1000,
};

/**
 * Download policy (#520), interpolated into the generated installer.
 *
 * Every configured source gets `attemptsPerSource` tries (the first is the
 * initial attempt, the rest are retries): official first, then each mirror.
 * With one mirror that is four attempts in total — three retries.
 */
export const DOWNLOAD = {
  attemptsPerSource: 2,
  retryBackoffMs: 1000,
  idleTimeoutMs: 30 * 1000,
  connectTimeoutMs: 15 * 1000,
};

const FS_ANCHOR = `const {
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtemp,
  rmSync,
} = require("fs");`;

const FS_REPLACEMENT = `const {
  chmodSync,
  copyFileSync,
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtemp,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} = require("fs");`;

const PATH_ANCHOR = `const { join, sep } = require("path");`;

const PATH_REPLACEMENT = `const { basename, dirname, join, sep } = require("path");`;

const OS_ANCHOR = `const { tmpdir } = require("os");`;

const OS_REPLACEMENT = `const { hostname, tmpdir } = require("os");`;

const CLASS_ANCHOR = "class Package {";

const HELPERS = `// [mint #504/#514] Install lock + staged commit helpers.
//
// The binary lives in a directory shared by every mint invocation of the same
// package ("node_modules/.bin_real"). The upstream installer deleted that
// directory and unpacked into it without any coordination, so two concurrent
// first runs (lazy \`run()\` or \`postinstall\`) destroyed each other's work.
// Installs are serialized by a lock directory, unpacked next to the final
// directory (same filesystem, so the commit cannot fail with EXDEV) and only
// published by an atomic rename.
const INSTALL_LOCK_RETRY_MS = ${LOCK.retryMs};
const INSTALL_LOCK_WARN_MS = ${LOCK.warnMs};
// Age fallback, used only when the lock owner cannot be identified; it must stay
// below INSTALL_LOCK_TIMEOUT_MS or the steal branch is unreachable (#514).
const INSTALL_LOCK_STALE_MS = ${LOCK.staleMs};
const INSTALL_LOCK_TIMEOUT_MS = ${LOCK.timeoutMs};

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) {
    return false;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    // EPERM means the process exists but belongs to another user.
    return Boolean(e && e.code === "EPERM");
  }
}

function readLockOwner(lockDir) {
  try {
    const owner = JSON.parse(readFileSync(join(lockDir, "owner.json"), "utf8"));
    return owner && Number.isInteger(owner.pid) ? owner : null;
  } catch {
    return null;
  }
}

function installLockIsStale(lockDir) {
  let ageMs = Infinity;
  try {
    ageMs = Date.now() - statSync(lockDir).mtimeMs;
  } catch {
    return true; // the lock disappeared; let the caller retry mkdir
  }
  const owner = readLockOwner(lockDir);
  if (owner) {
    // A live owner always wins; a dead owner is stolen immediately, so a lock
    // leaked by a crashed process cannot burn the whole wait timeout (#514).
    return !isProcessAlive(owner.pid);
  }
  return ageMs > INSTALL_LOCK_STALE_MS;
}

function acquireInstallLock(lockDir) {
  const startedAt = Date.now();
  const deadline = startedAt + INSTALL_LOCK_TIMEOUT_MS;
  return new Promise((resolve, reject) => {
    let warned = false;
    const attempt = () => {
      try {
        // mkdir(2) is atomic: whoever creates the directory owns the install.
        mkdirSync(lockDir);
        try {
          writeFileSync(
            join(lockDir, "owner.json"),
            JSON.stringify({
              pid: process.pid,
              startedAt: Date.now(),
              host: hostname(),
            }),
          );
        } catch {
          // best effort: without the owner file the age threshold decides
        }
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
      if (installLockIsStale(lockDir)) {
        try {
          rmSync(lockDir, { recursive: true, force: true });
        } catch {
          // best effort
        }
        setTimeout(attempt, 0);
        return;
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

/** Remove staging directories left behind by a crashed installer (#514). */
function cleanStaleStaging(finalDir) {
  const parent = dirname(finalDir);
  const prefix = \`\${basename(finalDir)}.staging-\`;
  try {
    for (const name of readdirSync(parent)) {
      if (!name.startsWith(prefix)) {
        continue;
      }
      try {
        rmSync(join(parent, name), { recursive: true, force: true });
      } catch {
        // best effort
      }
    }
  } catch {
    // best effort
  }
}

function copyTreeSync(from, to) {
  const info = statSync(from);
  if (info.isDirectory()) {
    mkdirSync(to, { recursive: true });
    for (const name of readdirSync(from)) {
      copyTreeSync(join(from, name), join(to, name));
    }
    return;
  }
  copyFileSync(from, to);
  try {
    chmodSync(to, info.mode & 0o777);
  } catch {
    // best effort (a platform may not support chmod)
  }
}

function commitStagedInstall(pkg, finalDir, stagingDir) {
  rmSync(finalDir, { recursive: true, force: true });
  try {
    renameSync(stagingDir, finalDir);
  } catch (e) {
    if (e.code !== "EXDEV" && e.code !== "EPERM") {
      throw e;
    }
    // Cross-device (or a Windows volume edge): copy instead of renaming. The
    // staging directory normally sits next to the final one, so this is a
    // safety net rather than the primary path (#514).
    copyTreeSync(stagingDir, finalDir);
    rmSync(stagingDir, { recursive: true, force: true });
  }
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

// [mint #520] Download policy: the official release URL first, then the mirrors
// recorded in the package metadata; every source gets one initial attempt plus
// retries, and a failing source falls through to the next one.
const DOWNLOAD_ATTEMPTS_PER_SOURCE = ${DOWNLOAD.attemptsPerSource};
const DOWNLOAD_RETRY_BACKOFF_MS = ${DOWNLOAD.retryBackoffMs};
const DOWNLOAD_IDLE_TIMEOUT_MS = ${DOWNLOAD.idleTimeoutMs};
const DOWNLOAD_CONNECT_TIMEOUT_MS = ${DOWNLOAD.connectTimeoutMs};

function downloadSources(pkg) {
  const urls = Array.isArray(pkg.downloadUrls) ? pkg.downloadUrls : [];
  const candidates = urls.length > 0 ? urls : [pkg.url];
  return candidates.filter((url) => typeof url === "string" && url.length > 0);
}

function sourceLabel(index) {
  return index === 0 ? "official" : \`mirror \${index}\`;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** HTTP 4xx other than 429 is a property of the source, not a transient error. */
function isRetryableDownloadError(e) {
  const match = /^HTTP (\\d{3})\\b/.exec(e && e.message ? e.message : "");
  if (!match) {
    return true; // network-level failure: socket hang up, timeout, DNS, TLS
  }
  const status = Number(match[1]);
  return status >= 500 || status === 429;
}

function resetStaging(pkg) {
  const stagingDir = pkg.installDirectory;
  rmSync(stagingDir, { recursive: true, force: true });
  mkdirSync(stagingDir, { recursive: true });
}

function downloadFailureReport(pkg, failures) {
  const lines = [
    \`all download sources failed for \${pkg.filename || pkg.name}\`,
  ];
  failures.forEach((failure, index) => {
    const attempts = \`\${failure.attempts} attempt\${failure.attempts === 1 ? "" : "s"}\`;
    lines.push(\`  [\${sourceLabel(index)}] \${failure.url}: \${failure.reason} (\${attempts})\`);
  });
  lines.push(
    "hint: retry, set HTTPS_PROXY/NO_PROXY, or check the mirror list in the package metadata",
  );
  return lines.join("\\n");
}

function fetchFromAnySource(pkg, sources, suppressLogs) {
  const failures = [];
  const attemptSource = (index) => {
    const url = sources[index];
    const failure = { url, reason: "unknown error", attempts: 0 };
    const tryOnce = () => {
      failure.attempts += 1;
      return pkg.fetchFrom(url, suppressLogs).then(
        () => true,
        (e) => {
          failure.reason = e && e.message ? e.message : String(e);
          if (
            !isRetryableDownloadError(e) ||
            failure.attempts >= DOWNLOAD_ATTEMPTS_PER_SOURCE
          ) {
            failures.push(failure);
            return false;
          }
          resetStaging(pkg);
          return sleep(DOWNLOAD_RETRY_BACKOFF_MS * failure.attempts).then(tryOnce);
        },
      );
    };
    return tryOnce().then((ok) => {
      if (ok) {
        return undefined;
      }
      if (index + 1 < sources.length) {
        resetStaging(pkg);
        return attemptSource(index + 1);
      }
      throw new Error(downloadFailureReport(pkg, failures));
    });
  };
  return Promise.resolve().then(() => attemptSource(0));
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
    }).catch((e) => {
      // withInstallLock settles (and releases the lock) before this handler
      // runs, so exiting here cannot leak the lock (#514).
      error(e.message);
    });
  }

  installStaged(suppressLogs) {
    // Unpack into a private staging directory next to the final one -- same
    // filesystem, so the commit rename cannot fail with EXDEV -- and publish it
    // only once it is complete. Readers never observe a half-written binary and
    // a failed install never destroys a working one.
    const finalDir = this.installDirectory;
    cleanStaleStaging(finalDir);
    const stagingDir = mkdtempSync(\`\${finalDir}.staging-\`);
    mkdirSync(stagingDir, { recursive: true });
    this.installDirectory = stagingDir;
    const sources = downloadSources(this);
    return fetchFromAnySource(this, sources, suppressLogs).then(
      () => {
        try {
          commitStagedInstall(this, finalDir, stagingDir);
        } catch (e) {
          abortStagedInstall(this, finalDir, stagingDir);
          throw e;
        }
        if (!suppressLogs) {
          console.error(\`\${this.name} has been installed!\`);
        }
      },
      (e) => {
        abortStagedInstall(this, finalDir, stagingDir);
        throw new Error(\`Error fetching release: \${e.message}\`);
      },
    );
  }

  fetchFrom(url, suppressLogs) {
    this.url = url;`;

const INSTALL_TAIL_ANCHOR = `      .then(() => {
        if (!suppressLogs) {
          console.error(\`\${this.name} has been installed!\`);
        }
      })
      .catch((e) => {
        error(\`Error fetching release: \${e.message}\`);
      });`;

const INSTALL_TAIL_REPLACEMENT = `      .then(() => {});`;

const DOWNLOAD_IDLE_ANCHOR = `      req.on("error", reject);
      req.end();`;

const DOWNLOAD_IDLE_REPLACEMENT = `      req.setTimeout(DOWNLOAD_IDLE_TIMEOUT_MS, () => {
        // A stalled body must fail the attempt instead of hanging forever.
        req.destroy(
          new Error(
            \`download stalled: no data for \${DOWNLOAD_IDLE_TIMEOUT_MS}ms\`,
          ),
        );
      });
      req.on("error", reject);
      req.end();`;

const PROXY_CONNECT_ANCHOR = `    connectReq.on("error", reject);
    connectReq.end();`;

const PROXY_CONNECT_REPLACEMENT = `    connectReq.setTimeout(DOWNLOAD_CONNECT_TIMEOUT_MS, () => {
      connectReq.destroy(
        new Error(
          \`proxy connect timed out after \${DOWNLOAD_CONNECT_TIMEOUT_MS}ms\`,
        ),
      );
    });
    connectReq.on("error", reject);
    connectReq.end();`;

const PATCHES = [
  { name: "fs imports", from: FS_ANCHOR, to: FS_REPLACEMENT },
  { name: "path imports", from: PATH_ANCHOR, to: PATH_REPLACEMENT },
  { name: "os imports", from: OS_ANCHOR, to: OS_REPLACEMENT },
  { name: "class Package", from: CLASS_ANCHOR, to: HELPERS },
  { name: "install() head", from: INSTALL_HEAD_ANCHOR, to: INSTALL_HEAD_REPLACEMENT },
  { name: "install() tail", from: INSTALL_TAIL_ANCHOR, to: INSTALL_TAIL_REPLACEMENT },
  { name: "download() idle timeout", from: DOWNLOAD_IDLE_ANCHOR, to: DOWNLOAD_IDLE_REPLACEMENT },
  { name: "proxy connect timeout", from: PROXY_CONNECT_ANCHOR, to: PROXY_CONNECT_REPLACEMENT },
];

/**
 * Rewrite cargo-dist's generated `binary-install.js`.
 *
 * @param {string} source
 * @returns {{code: string, changed: boolean, missing: string[], ambiguous: string[]}}
 */
export function patchBinaryInstall(source) {
  return applyPatches(source, PATCHES);
}

export const BINARY_FILENAME = "binary.js";

const BINARY_FALLBACK_ANCHOR = `// FIXME: implement NPM installer handling of fallback download URLs
const artifactDownloadUrl = artifactDownloadUrls[0];`;

const BINARY_FALLBACK_REPLACEMENT = `// [mint #520] every configured source is tried in order by the installer.
const artifactDownloadUrl = artifactDownloadUrls[0];
const artifactDownloadUrlList = artifactDownloadUrls.slice();`;

const BINARY_GET_PACKAGE_ANCHOR = `  const url = \`\${artifactDownloadUrl}/\${platform.artifactName}\`;
  let filename = platform.artifactName;
  let ext = platform.zipExt;
  let binary = new Package(platform, name, url, filename, ext, platform.bins);`;

const BINARY_GET_PACKAGE_REPLACEMENT = `  const urls = artifactDownloadUrlList.map(
    (base) => \`\${base}/\${platform.artifactName}\`,
  );
  const url = urls[0];
  let filename = platform.artifactName;
  let ext = platform.zipExt;
  let binary = new Package(platform, name, url, filename, ext, platform.bins);
  binary.downloadUrls = urls;`;

const BINARY_PATCHES = [
  { name: "binary.js fallback URLs", from: BINARY_FALLBACK_ANCHOR, to: BINARY_FALLBACK_REPLACEMENT },
  { name: "binary.js getPackage", from: BINARY_GET_PACKAGE_ANCHOR, to: BINARY_GET_PACKAGE_REPLACEMENT },
];

/**
 * Rewrite cargo-dist's generated `binary.js` so the installer knows every
 * configured download source instead of only `artifactDownloadUrls[0]` (#520).
 *
 * @param {string} source
 * @returns {{code: string, changed: boolean, missing: string[], ambiguous: string[]}}
 */
export function patchBinaryJs(source) {
  return applyPatches(source, BINARY_PATCHES);
}

/** Apply an anchor patch set, reporting missing/ambiguous anchors instead of guessing. */
export function applyPatches(source, patches) {
  if (source.includes(MARKER)) {
    return { code: source, changed: false, missing: [], ambiguous: [] };
  }
  const missing = [];
  const ambiguous = [];
  for (const patch of patches) {
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
  for (const patch of patches) {
    code = code.replace(patch.from, patch.to);
  }
  code = `${MARKER}\n${code}`;
  return { code, changed: true, missing: [], ambiguous: [] };
}

/** Patched files of one npm package, in patch order. */
export const PATCH_TARGETS = [
  { filename: INSTALLER_FILENAME, patch: patchBinaryInstall },
  { filename: BINARY_FILENAME, patch: patchBinaryJs },
];

/** Throws when `code` is not valid CommonJS JavaScript. */
export function assertSyntax(code, filename = INSTALLER_FILENAME) {
  new Script(code, { filename });
}

/** Human-readable description of a failed anchor patch. */
export function describeFailure(missing, ambiguous) {
  const parts = [];
  if (missing.length > 0) {
    parts.push(`missing anchors: ${missing.join(", ")}`);
  }
  if (ambiguous.length > 0) {
    parts.push(`ambiguous anchors: ${ambiguous.join(", ")}`);
  }
  return parts.join("; ");
}
