// Behaviour tests for #504: concurrent first installs into the same package
// directory must all succeed, and an install in flight must not touch the
// existing install directory until it can be committed atomically.
//
// Fully local: the release archive is served over loopback HTTP, so no network
// access and no real mint-faa release are involved.

import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

import { patchBinaryInstall } from "./patch-installer.mjs";

const FIXTURE = fileURLToPath(
  new URL("./fixtures/binary-install.mint-faa-0.8.0.js", import.meta.url),
);

const ROUNDS = 3;
const CONCURRENCY = 4;
const BINARY = "#!/bin/sh\necho mint-ok\n";
const STAGING_PREFIX = "mint-faa-install-";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

mkdirSync(tmpdir(), { recursive: true });
const scratch = mkdtempSync(join(tmpdir(), "mint-npm-race-test-"));

const stagingRoot = join(scratch, "staging");
const pkgDir = join(scratch, "pkg");
const installDir = join(pkgDir, "node_modules", ".bin_real");
const lockDir = `${installDir}.lock`;
mkdirSync(stagingRoot, { recursive: true });
mkdirSync(pkgDir, { recursive: true });

const patched = patchBinaryInstall(readFileSync(FIXTURE, "utf8"));
assert.equal(patched.changed, true, "the fixture must be patchable");
writeFileSync(join(pkgDir, "binary-install.js"), patched.code);
writeFileSync(
  join(pkgDir, "driver.cjs"),
  `const { Package } = require("./binary-install.js");
const pkg = new Package({}, "mint-faa", process.argv[2], "mint-fixture.tar.gz", ".tar.gz", { mint: "mint" });
pkg.install(true).then(
  () => process.exit(0),
  (e) => {
    console.error(e && e.message);
    process.exit(1);
  },
);
`,
);

// Build the release archive the installer unpacks: one executable `mint` entry
// below a leading directory component (stripped by `tar --strip-components 1`).
const archiveRoot = join(scratch, "archive");
const tarball = join(scratch, "mint-fixture.tar.gz");
mkdirSync(join(archiveRoot, "mint-faa-0.0.0"), { recursive: true });
const fixtureBinary = join(archiveRoot, "mint-faa-0.0.0", "mint");
writeFileSync(fixtureBinary, BINARY);
chmodSync(fixtureBinary, 0o755);
execFileSync("tar", ["czf", tarball, "-C", archiveRoot, "mint-faa-0.0.0"]);
const archive = readFileSync(tarball);

let responseDelayMs = 300;
const server = createServer((_req, res) => {
  // Slow enough that callers are inside the download path while the test looks
  // at the filesystem.
  setTimeout(() => {
    res.writeHead(200, { "content-type": "application/octet-stream" });
    res.end(archive);
  }, responseDelayMs);
});
let url = "";

before(async () => {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  url = `http://127.0.0.1:${server.address().port}/mint-fixture.tar.gz`;
});

after(() => {
  server.close();
  rmSync(scratch, { recursive: true, force: true });
});

function runChild() {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [join(pkgDir, "driver.cjs"), url], {
      cwd: pkgDir,
      env: { ...process.env, TMPDIR: stagingRoot },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), 60_000);
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
  });
}

function resetInstallDir() {
  rmSync(join(pkgDir, "node_modules"), { recursive: true, force: true });
}

function assertFreshInstall(round) {
  const binary = join(installDir, "mint");
  assert.ok(existsSync(binary), `binary missing in round ${round}`);
  assert.equal(readFileSync(binary, "utf8"), BINARY);

  assert.ok(
    statSync(binary).mode & 0o111,
    "binary must be executable",
  );

  const executed = spawnSync(binary, []);
  assert.equal(executed.status, 0);
  assert.equal(executed.stdout.toString(), "mint-ok\n");

  assert.equal(existsSync(lockDir), false, "lock directory must be released");
  const leftovers = readdirSync(stagingRoot).filter((name) =>
    name.startsWith(STAGING_PREFIX),
  );
  assert.deepEqual(leftovers, [], "staging directories must be cleaned up");
}

test("an in-flight install leaves the existing install directory untouched", async () => {
  // A dir that is not a complete install (no `mint`) forces the install path,
  // while its content is what an interrupted/concurrent install must preserve
  // until it can commit atomically.
  resetInstallDir();
  mkdirSync(installDir, { recursive: true });
  const sentinel = join(installDir, "README.md");
  writeFileSync(sentinel, "previous release\n");

  responseDelayMs = 800;
  const child = runChild();
  await sleep(250); // inside the download window
  assert.equal(
    existsSync(sentinel),
    true,
    "the install directory must not be destroyed while the download is in flight",
  );
  assert.equal(existsSync(lockDir), true, "the install must hold the lock");

  const result = await child;
  responseDelayMs = 300;
  assert.equal(result.code, 0, `child failed: ${result.stderr}`);
  assertFreshInstall("in-flight");
  assert.equal(existsSync(sentinel), false, "the commit replaces the directory");
});

test("concurrent first installs share one lock and commit atomically", async () => {
  for (let round = 0; round < ROUNDS; round += 1) {
    resetInstallDir();
    const results = await Promise.all(
      Array.from({ length: CONCURRENCY }, () => runChild()),
    );
    for (const result of results) {
      assert.equal(
        result.code,
        0,
        `child failed (round ${round}): ${result.stderr}\n${result.stdout}`,
      );
    }
    assertFreshInstall(round);
  }
});
