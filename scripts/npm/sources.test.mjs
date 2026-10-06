// Behaviour tests for #520: the npm installer must retry a failing source,
// fall through to the next configured source (`artifactDownloadUrls`), never
// retry a non-transient HTTP status, and report exactly one diagnostic line per
// source when every source failed.
//
// Fully local: the sources are loopback HTTP servers, so no network access and
// no real mint-faa release are involved.

import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

import { patchBinaryInstall, patchBinaryJs } from "./patch-installer.mjs";

const INSTALLER_FIXTURE = fileURLToPath(
  new URL("./fixtures/binary-install.mint-faa-0.8.0.js", import.meta.url),
);
const BINARY_FIXTURE = fileURLToPath(
  new URL("./fixtures/binary.mint-faa-0.8.1.js", import.meta.url),
);

const BINARY = "#!/bin/sh\necho mint-ok\n";
const ARTIFACT_NAME = "mint-faa-fixture.tar.gz";
const skipOnWindows = process.platform === "win32";

mkdirSync(tmpdir(), { recursive: true });
const scratch = mkdtempSync(join(tmpdir(), "mint-npm-sources-test-"));

const pkgDir = join(scratch, "pkg");
const nodeModules = join(pkgDir, "node_modules");
const installDir = join(nodeModules, ".bin_real");
const childTmp = join(scratch, "tmp");
mkdirSync(pkgDir, { recursive: true });
mkdirSync(childTmp, { recursive: true });

/** Rust target triple as `binary.js` derives it, with a stubbed glibc probe. */
function hostTriple() {
  const arch = process.arch === "x64" ? "x86_64" : process.arch === "arm64" ? "aarch64" : null;
  if (arch === null) {
    return null;
  }
  if (process.platform === "darwin") {
    return `${arch}-apple-darwin`;
  }
  if (process.platform === "linux") {
    return `${arch}-unknown-linux-gnu`;
  }
  return null;
}

const triple = hostTriple();

const patchedInstaller = patchBinaryInstall(readFileSync(INSTALLER_FIXTURE, "utf8"));
assert.equal(patchedInstaller.changed, true);
const patchedBinary = patchBinaryJs(readFileSync(BINARY_FIXTURE, "utf8"));
assert.equal(patchedBinary.changed, true);

// A release archive with a leading directory component, like cargo-dist's.
const archiveRoot = join(scratch, "archive");
const tarball = join(scratch, ARTIFACT_NAME);
mkdirSync(join(archiveRoot, "mint-faa-0.0.0"), { recursive: true });
const fixtureBinary = join(archiveRoot, "mint-faa-0.0.0", "mint");
writeFileSync(fixtureBinary, BINARY);
chmodSync(fixtureBinary, 0o755);
execFileSync("tar", ["czf", tarball, "-C", archiveRoot, "mint-faa-0.0.0"]);
const archive = readFileSync(tarball);

// A second, still perfectly valid archive with different bytes: it exercises the
// digest check without tripping the tar step first.
const evilRoot = join(scratch, "evil");
const evilTarball = join(scratch, `evil-${ARTIFACT_NAME}`);
mkdirSync(join(evilRoot, "mint-faa-0.0.0"), { recursive: true });
const evilBinary = join(evilRoot, "mint-faa-0.0.0", "mint");
writeFileSync(evilBinary, "#!/bin/sh\necho mint-evil\n");
chmodSync(evilBinary, 0o755);
execFileSync("tar", ["czf", evilTarball, "-C", evilRoot, "mint-faa-0.0.0"]);
const evilArchive = readFileSync(evilTarball);

/** One fake download source: mode is "ok", "500" or "404". */
function makeSource(mode) {
  const state = { hits: 0, mode, payload: archive };
  const server = createServer((_req, res) => {
    state.hits += 1;
    if (state.mode === "500") {
      res.writeHead(500, { "content-type": "text/plain" });
      res.end("boom");
      return;
    }
    if (state.mode === "404") {
      res.writeHead(404, { "content-type": "text/plain" });
      res.end("nope");
      return;
    }
    res.writeHead(200, { "content-type": "application/octet-stream" });
    res.end(state.payload);
  });
  return { state, server };
}

const official = makeSource("500");
const mirror = makeSource("500");

before(async () => {
  for (const source of [official, mirror]) {
    await new Promise((resolve) => source.server.listen(0, "127.0.0.1", resolve));
    source.base = `http://127.0.0.1:${source.server.address().port}/download`;
  }
  writeFileSync(
    join(pkgDir, "driver.cjs"),
    `// Stub detect-libc: the test package targets this host's triple.
const Module = require("module");
const realLoad = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === "detect-libc") {
    return {
      familySync: () => "glibc",
      isNonGlibcLinuxSync: () => false,
      versionSync: () => "2.39",
    };
  }
  return realLoad.call(this, request, parent, isMain);
};
const { install } = require("./binary.js");
Promise.resolve(install(true)).then(
  () => process.exit(0),
  (e) => {
    console.error(e && e.message);
    process.exit(1);
  },
);
`,
  );
});

after(() => {
  for (const source of [official, mirror]) {
    source.server.close();
  }
  rmSync(scratch, { recursive: true, force: true });
});

function writePackage(sources, extra = {}) {
  rmSync(nodeModules, { recursive: true, force: true });
  writeFileSync(join(pkgDir, "binary-install.js"), patchedInstaller.code);
  writeFileSync(join(pkgDir, "binary.js"), patchedBinary.code);
  writeFileSync(
    join(pkgDir, "package.json"),
    JSON.stringify(
      {
        name: "mint-faa",
        version: "0.0.0",
        artifactDownloadUrls: sources,
        supportedPlatforms: {
          [triple]: {
            artifactName: ARTIFACT_NAME,
            bins: { mint: "mint" },
            zipExt: ".tar.gz",
          },
        },
        glibcMinimum: { major: 2, series: 31 },
        ...extra,
      },
      null,
      2,
    ),
  );
}

function runChild() {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [join(pkgDir, "driver.cjs")], {
      cwd: pkgDir,
      env: { ...process.env, TMPDIR: childTmp },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), 60_000);
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code, stderr });
    });
  });
}

function resetHits() {
  official.state.hits = 0;
  mirror.state.hits = 0;
}

function digestOf(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function assertInstalled() {
  const binary = join(installDir, "mint");
  assert.ok(existsSync(binary), "the binary must be installed");
  assert.equal(readFileSync(binary, "utf8"), BINARY);
  assert.ok(statSync(binary).mode & 0o111);
  assert.equal(existsSync(`${installDir}.lock`), false, "the lock must be released");
  assert.deepEqual(
    existsSync(nodeModules) ? readdirSync(nodeModules).filter((n) => n.startsWith(".bin_real.staging-")) : [],
    [],
    "staging must be cleaned up",
  );
}

test("falls back to the mirror after the official source fails twice", { skip: skipOnWindows || triple === null }, async () => {
  official.state.mode = "500";
  mirror.state.mode = "ok";
  resetHits();
  writePackage([official.base, mirror.base]);

  const result = await runChild();
  assert.equal(result.code, 0, `child failed: ${result.stderr}`);
  assert.equal(official.state.hits, 2, "the official source gets one attempt plus one retry");
  assert.equal(mirror.state.hits, 1, "the mirror is tried once it is reached");
  assertInstalled();
});

test("a 404 switches source instead of retrying it", { skip: skipOnWindows || triple === null }, async () => {
  official.state.mode = "404";
  mirror.state.mode = "ok";
  resetHits();
  writePackage([official.base, mirror.base]);

  const result = await runChild();
  assert.equal(result.code, 0, `child failed: ${result.stderr}`);
  assert.equal(official.state.hits, 1, "a 4xx is not retried");
  assert.equal(mirror.state.hits, 1);
  assertInstalled();
});

test("reports one line per source when every source fails", { skip: skipOnWindows || triple === null }, async () => {
  official.state.mode = "500";
  mirror.state.mode = "404";
  resetHits();
  writePackage([official.base, mirror.base]);

  const result = await runChild();
  assert.equal(result.code, 1, "a failed install must exit non-zero");
  assert.equal(official.state.hits, 2);
  assert.equal(mirror.state.hits, 1);
  assert.equal(
    result.stderr.split("\n").filter((line) => line.trim().startsWith("[")).length,
    2,
    `expected one diagnostic line per source, got:\n${result.stderr}`,
  );
  assert.match(result.stderr, /\[official\] .*HTTP 500 .*\(2 attempts\)/);
  assert.match(result.stderr, /\[mirror 1\] .*HTTP 404 .*\(1 attempt\)/);
  assert.match(result.stderr, /hint: /);
  assert.equal(
    existsSync(join(installDir, "mint")),
    false,
    "no binary may be committed after a failed install",
  );
  assert.equal(existsSync(`${installDir}.lock`), false, "the lock must be released");
});

test("refuses a download whose sha256 does not match the package metadata", { skip: skipOnWindows || triple === null }, async () => {
  // Both archives are valid tarballs, so only the digest check can catch this.
  official.state.mode = "ok";
  official.state.payload = evilArchive;
  mirror.state.mode = "500";
  resetHits();
  writePackage([official.base], {
    artifactSha256: { [ARTIFACT_NAME]: digestOf(archive) },
  });

  const result = await runChild();
  assert.equal(result.code, 1, "a tampered download must fail the install");
  assert.match(result.stderr, /sha256 mismatch/);
  assert.equal(official.state.hits, 2, "a digest mismatch is retried");
  assert.equal(
    existsSync(join(installDir, "mint")),
    false,
    "nothing may be committed from a tampered download",
  );
  assert.equal(existsSync(`${installDir}.lock`), false, "the lock must be released");
});

test("warns but installs when the package records no digest", { skip: skipOnWindows || triple === null }, async () => {
  official.state.mode = "ok";
  official.state.payload = archive;
  resetHits();
  writePackage([official.base]);

  const result = await runChild();
  assert.equal(result.code, 0, `child failed: ${result.stderr}`);
  assert.match(result.stderr, /no sha256 recorded/);
  assertInstalled();
});
