// Unit tests for scripts/npm/patch-installer.mjs (#504).

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  MARKER,
  assertSyntax,
  patchBinaryInstall,
} from "./patch-installer.mjs";

const FIXTURE = fileURLToPath(
  new URL("./fixtures/binary-install.mint-faa-0.8.0.js", import.meta.url),
);
const SCRIPT = fileURLToPath(new URL("./patch-installer.mjs", import.meta.url));

const fixtureSource = readFileSync(FIXTURE, "utf8");
const patched = patchBinaryInstall(fixtureSource);

mkdirSync(tmpdir(), { recursive: true });
const scratch = mkdtempSync(join(tmpdir(), "mint-npm-patch-test-"));

after(() => {
  rmSync(scratch, { recursive: true, force: true });
});

test("patches the cargo-dist installer fixture", () => {
  assert.equal(patched.changed, true);
  assert.deepEqual(patched.missing, []);
  assert.deepEqual(patched.ambiguous, []);
  assert.ok(patched.code.includes(MARKER));
  assert.ok(patched.code.includes("function acquireInstallLock"));
  assert.ok(patched.code.includes("installStaged(suppressLogs)"));
  assert.ok(
    patched.code.includes("commitStagedInstall(this, finalDir, stagingDir)"),
  );
  assert.ok(patched.code.includes("abortStagedInstall(this, finalDir, stagingDir)"));
  // The unguarded install-directory deletion must be gone.
  assert.equal(patched.code.includes("rmSync(this.installDirectory"), false);
  // Upstream's own logic is preserved verbatim.
  assert.ok(patched.code.includes("module.exports.Package = Package;"));
  assert.ok(patched.code.includes('spawnSync("tar", ['));
});

test("patched output is valid CommonJS", () => {
  assert.doesNotThrow(() => assertSyntax(patched.code));
  assert.throws(() => assertSyntax("function ("));
});

test("patching is idempotent", () => {
  const again = patchBinaryInstall(patched.code);
  assert.equal(again.changed, false);
  assert.equal(again.code, patched.code);
  assert.deepEqual(again.missing, []);
});

test("missing anchors are reported instead of silently skipped", () => {
  const result = patchBinaryInstall("// nothing cargo-dist generated here\n");
  assert.equal(result.changed, false);
  assert.equal(result.code, "// nothing cargo-dist generated here\n");
  assert.ok(result.missing.includes("fs imports"));
  assert.ok(result.missing.includes("install() head"));
});

test("ambiguous anchors are reported", () => {
  const doubled = `${fixtureSource}\n${fixtureSource}`;
  const result = patchBinaryInstall(doubled);
  assert.equal(result.changed, false);
  assert.ok(result.ambiguous.includes("fs imports"));
});

test("CLI patches a package directory, then --check succeeds", () => {
  const pkgDir = join(scratch, "pkg");
  mkdirSync(pkgDir, { recursive: true });
  const installer = join(pkgDir, "binary-install.js");
  writeFileSync(installer, fixtureSource);

  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, pkgDir, "--check"], { stdio: "pipe" });
  });
  execFileSync(process.execPath, [SCRIPT, pkgDir], { stdio: "pipe" });
  execFileSync(process.execPath, [SCRIPT, pkgDir, "--check"], { stdio: "pipe" });
  execFileSync(process.execPath, [SCRIPT, installer], { stdio: "pipe" });
  assert.ok(readFileSync(installer, "utf8").includes(MARKER));
});

test("CLI refuses a file whose anchors are gone", () => {
  const brokenDir = join(scratch, "broken");
  mkdirSync(brokenDir, { recursive: true });
  writeFileSync(join(brokenDir, "binary-install.js"), "// not the installer\n");
  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, brokenDir], { stdio: "pipe" });
  });
});
