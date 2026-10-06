// Unit tests for scripts/npm/patch-installer.mjs (#504 / #514 / #520).

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
  patchBinaryJs,
} from "./patch-installer.mjs";

const FIXTURE = fileURLToPath(
  new URL("./fixtures/binary-install.mint-faa-0.8.0.js", import.meta.url),
);
const BINARY_FIXTURE = fileURLToPath(
  new URL("./fixtures/binary.mint-faa-0.8.1.js", import.meta.url),
);
const SCRIPT = fileURLToPath(new URL("./patch-installer.mjs", import.meta.url));

const fixtureSource = readFileSync(FIXTURE, "utf8");
const binaryFixtureSource = readFileSync(BINARY_FIXTURE, "utf8");
const patched = patchBinaryInstall(fixtureSource);
const patchedBinary = patchBinaryJs(binaryFixtureSource);

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
  assert.ok(patched.code.includes("function fetchFromAnySource"));
  assert.ok(patched.code.includes("req.setTimeout(DOWNLOAD_IDLE_TIMEOUT_MS"));
  // The unguarded install-directory deletion must be gone.
  assert.equal(patched.code.includes("rmSync(this.installDirectory"), false);
  // Upstream's own logic is preserved verbatim.
  assert.ok(patched.code.includes("module.exports.Package = Package;"));
  assert.ok(patched.code.includes('spawnSync("tar", ['));
});

test("patches the cargo-dist binary.js fixture", () => {
  assert.equal(patchedBinary.changed, true);
  assert.deepEqual(patchedBinary.missing, []);
  assert.deepEqual(patchedBinary.ambiguous, []);
  assert.ok(patchedBinary.code.includes(MARKER));
  assert.ok(patchedBinary.code.includes("const artifactDownloadUrlList"));
  assert.ok(patchedBinary.code.includes("binary.downloadUrls = urls;"));
  assert.equal(patchedBinary.code.includes("FIXME"), false);
  // Upstream's platform handling is untouched.
  assert.ok(patchedBinary.code.includes("const getPlatform = () => {"));
});

test("patched output is valid CommonJS", () => {
  assert.doesNotThrow(() => assertSyntax(patched.code));
  assert.doesNotThrow(() => assertSyntax(patchedBinary.code, "binary.js"));
  assert.throws(() => assertSyntax("function ("));
});

test("patching is idempotent", () => {
  const again = patchBinaryInstall(patched.code);
  assert.equal(again.changed, false);
  assert.equal(again.code, patched.code);
  assert.deepEqual(again.missing, []);

  const againBinary = patchBinaryJs(patchedBinary.code);
  assert.equal(againBinary.changed, false);
  assert.equal(againBinary.code, patchedBinary.code);
});

test("missing anchors are reported instead of silently skipped", () => {
  const result = patchBinaryInstall("// nothing cargo-dist generated here\n");
  assert.equal(result.changed, false);
  assert.equal(result.code, "// nothing cargo-dist generated here\n");
  assert.ok(result.missing.includes("fs imports"));
  assert.ok(result.missing.includes("install() head"));

  const binaryResult = patchBinaryJs("// nothing cargo-dist generated here\n");
  assert.equal(binaryResult.changed, false);
  assert.ok(binaryResult.missing.includes("binary.js fallback URLs"));
  assert.ok(binaryResult.missing.includes("binary.js getPackage"));
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
  const binary = join(pkgDir, "binary.js");
  writeFileSync(installer, fixtureSource);
  writeFileSync(binary, binaryFixtureSource);

  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, pkgDir, "--check"], { stdio: "pipe" });
  });
  execFileSync(process.execPath, [SCRIPT, pkgDir], { stdio: "pipe" });
  execFileSync(process.execPath, [SCRIPT, pkgDir, "--check"], { stdio: "pipe" });
  execFileSync(process.execPath, [SCRIPT, installer], { stdio: "pipe" });
  execFileSync(process.execPath, [SCRIPT, binary], { stdio: "pipe" });
  assert.ok(readFileSync(installer, "utf8").includes(MARKER));
  assert.ok(readFileSync(binary, "utf8").includes(MARKER));
});

test("CLI refuses a package directory whose anchors are gone", () => {
  const brokenDir = join(scratch, "broken");
  mkdirSync(brokenDir, { recursive: true });
  writeFileSync(join(brokenDir, "binary-install.js"), "// not the installer\n");
  writeFileSync(join(brokenDir, "binary.js"), "// not the launcher\n");
  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, brokenDir], { stdio: "pipe" });
  });
});

test("CLI rejects an unknown generated file", () => {
  const path = join(scratch, "other.js");
  writeFileSync(path, "// unrelated\n");
  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, path], { stdio: "pipe" });
  });
});
