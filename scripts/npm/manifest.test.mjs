// Unit tests for the release-time package metadata injection (#521):
// mirror sources appended to `artifactDownloadUrls` and the per-artifact
// sha256 recorded as `artifactSha256`.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  ARTIFACT_DIGESTS_KEY,
  MIRRORS_FILENAME,
  collectArtifactDigests,
  expandMirrorUrls,
  loadMirrorTemplates,
} from "./manifest.mjs";

const SCRIPT = fileURLToPath(new URL("./patch-installer.mjs", import.meta.url));
const MIRRORS = fileURLToPath(new URL("./mirrors.json", import.meta.url));
const INSTALLER_FIXTURE = fileURLToPath(
  new URL("./fixtures/binary-install.mint-faa-0.8.0.js", import.meta.url),
);
const BINARY_FIXTURE = fileURLToPath(
  new URL("./fixtures/binary.mint-faa-0.8.1.js", import.meta.url),
);

const OFFICIAL = "https://github.com/yanqd0/mint/releases/download/0.9.0";
const ARTIFACTS = {
  "mint-faa-aarch64-apple-darwin.tar.xz": "aarch64-darwin payload\n",
  "mint-faa-x86_64-unknown-linux-musl.tar.xz": "x86_64-linux payload\n",
};

mkdirSync(tmpdir(), { recursive: true });
const scratch = mkdtempSync(join(tmpdir(), "mint-npm-manifest-test-"));

after(() => {
  rmSync(scratch, { recursive: true, force: true });
});

/** Build a minimal unpacked package plus a directory with the release artifacts. */
function makePackage(name) {
  const pkgDir = join(scratch, name, "package");
  const artifactsDir = join(scratch, name, "artifacts");
  mkdirSync(pkgDir, { recursive: true });
  mkdirSync(artifactsDir, { recursive: true });
  writeFileSync(join(pkgDir, "binary-install.js"), readFileSync(INSTALLER_FIXTURE, "utf8"));
  writeFileSync(join(pkgDir, "binary.js"), readFileSync(BINARY_FIXTURE, "utf8"));
  writeFileSync(
    join(pkgDir, "package.json"),
    `${JSON.stringify(
      {
        name: "mint-faa",
        version: "0.9.0",
        artifactDownloadUrls: [OFFICIAL],
        supportedPlatforms: {
          "aarch64-apple-darwin": { artifactName: "mint-faa-aarch64-apple-darwin.tar.xz" },
          "x86_64-unknown-linux-musl": {
            artifactName: "mint-faa-x86_64-unknown-linux-musl.tar.xz",
          },
        },
        glibcMinimum: { major: 2, series: 31 },
      },
      null,
      2,
    )}\n`,
  );
  for (const [name_, content] of Object.entries(ARTIFACTS)) {
    writeFileSync(join(artifactsDir, name_), content);
  }
  return { pkgDir, artifactsDir };
}

function digest(content) {
  return createHash("sha256").update(content).digest("hex");
}

test("the checked-in mirror list carries a {url} placeholder", () => {
  const templates = loadMirrorTemplates(MIRRORS);
  assert.ok(templates.length >= 1);
  assert.ok(templates.every((template) => template.includes("{url}")));
  assert.ok(MIRRORS_FILENAME.endsWith(".json"));
});

test("mirror templates expand after every official URL", () => {
  const urls = expandMirrorUrls([OFFICIAL], ["https://gh-proxy.com/{url}"]);
  assert.deepEqual(urls, [OFFICIAL, `https://gh-proxy.com/${OFFICIAL}`]);
  // Idempotent: expanding an already-expanded list must not duplicate entries.
  assert.deepEqual(expandMirrorUrls(urls, ["https://gh-proxy.com/{url}"]), urls);
});

test("artifact digests cover every platform artifact", () => {
  const { pkgDir, artifactsDir } = makePackage("digests");
  const pkgJson = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));
  const digests = collectArtifactDigests(pkgJson, artifactsDir);
  assert.deepEqual(Object.keys(digests).sort(), Object.keys(ARTIFACTS).sort());
  assert.equal(digests["mint-faa-aarch64-apple-darwin.tar.xz"], digest(ARTIFACTS["mint-faa-aarch64-apple-darwin.tar.xz"]));
});

test("CLI injects mirrors and digests, then --check accepts the package", () => {
  const { pkgDir, artifactsDir } = makePackage("inject");
  execFileSync(
    process.execPath,
    [SCRIPT, pkgDir, "--checksums-from", artifactsDir],
    { stdio: "pipe" },
  );

  const pkgJson = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));
  assert.deepEqual(pkgJson.artifactDownloadUrls, [
    OFFICIAL,
    `https://gh-proxy.com/${OFFICIAL}`,
  ]);
  assert.equal(
    pkgJson[ARTIFACT_DIGESTS_KEY]["mint-faa-x86_64-unknown-linux-musl.tar.xz"],
    digest(ARTIFACTS["mint-faa-x86_64-unknown-linux-musl.tar.xz"]),
  );

  // Idempotent, and --check validates both the patch and the injected metadata.
  execFileSync(
    process.execPath,
    [SCRIPT, pkgDir, "--checksums-from", artifactsDir],
    { stdio: "pipe" },
  );
  execFileSync(
    process.execPath,
    [SCRIPT, pkgDir, "--check", "--checksums-from", artifactsDir],
    { stdio: "pipe" },
  );

  const again = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));
  assert.deepEqual(again.artifactDownloadUrls, pkgJson.artifactDownloadUrls);
});

test("--check rejects a tampered artifact", () => {
  const { pkgDir, artifactsDir } = makePackage("tampered");
  execFileSync(
    process.execPath,
    [SCRIPT, pkgDir, "--checksums-from", artifactsDir],
    { stdio: "pipe" },
  );
  writeFileSync(join(artifactsDir, "mint-faa-aarch64-apple-darwin.tar.xz"), "tampered\n");
  assert.throws(() => {
    execFileSync(
      process.execPath,
      [SCRIPT, pkgDir, "--check", "--checksums-from", artifactsDir],
      { stdio: "pipe" },
    );
  });
});

test("a missing platform artifact makes the release step fail", () => {
  const { pkgDir, artifactsDir } = makePackage("missing");
  rmSync(join(artifactsDir, "mint-faa-aarch64-apple-darwin.tar.xz"));
  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, pkgDir, "--checksums-from", artifactsDir], {
      stdio: "pipe",
    });
  });
});

test("injection flags need a package directory", () => {
  const { pkgDir } = makePackage("flags");
  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, join(pkgDir, "binary.js"), "--checksums-from", "."], {
      stdio: "pipe",
    });
  });
  assert.throws(() => {
    execFileSync(process.execPath, [SCRIPT, pkgDir, "--checksums-from"], { stdio: "pipe" });
  });
});
