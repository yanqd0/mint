#!/usr/bin/env node
// Rewrite cargo-dist's generated npm launcher so that a concurrent first
// install cannot destroy another process's install (#504), staging stays on the
// same filesystem as the install directory and every failure path releases the
// lock (#514), and downloads retry / fall back to a mirror (#520).
//
// Why a patch instead of a source file: `binary-install.js` and `binary.js` are
// not in this repository. cargo-dist renders them at release time into
// `mint-faa-<version>-npm-package.tar.gz`, and cargo-dist exposes no template
// override for them, so the release workflow runs this script on the unpacked
// package before `npm pack` / `npm publish` (see `.github/workflows/release.yml`).
//
// Usage:
//   node scripts/npm/patch-installer.mjs <package-dir|generated-file>
//   node scripts/npm/patch-installer.mjs <package-dir|generated-file> --check

import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import {
  MARKER,
  PATCH_TARGETS,
  assertSyntax,
  describeFailure,
} from "./installer-patches.mjs";

export {
  INSTALLER_FILENAME,
  MARKER,
  assertSyntax,
  patchBinaryInstall,
} from "./installer-patches.mjs";

const USAGE =
  "usage: node scripts/npm/patch-installer.mjs <package-dir|generated-file> [--check]";

/**
 * Resolve the patch list for a CLI target.
 *
 * @param {string} target package directory or one generated file
 * @returns {Array<{filename: string, patch: (source: string) => object, path: string}> | null}
 */
function resolveTargets(target) {
  if (existsSync(target) && statSync(target).isDirectory()) {
    return PATCH_TARGETS.map((entry) => ({
      filename: entry.filename,
      patch: entry.patch,
      path: join(target, entry.filename),
    }));
  }
  const entry = PATCH_TARGETS.find((candidate) => target.endsWith(candidate.filename));
  if (!entry) {
    return null;
  }
  return [{ filename: entry.filename, patch: entry.patch, path: target }];
}

function main(argv) {
  const args = argv.slice(2);
  const check = args.includes("--check");
  const targets = args.filter((arg) => !arg.startsWith("-"));
  if (targets.length !== 1) {
    console.error(USAGE);
    return 2;
  }
  const files = resolveTargets(targets[0]);
  if (files === null) {
    console.error(`unrecognized generated file: ${targets[0]}`);
    console.error(USAGE);
    return 2;
  }

  let failed = false;
  for (const file of files) {
    if (!existsSync(file.path)) {
      console.error(`not found: ${file.path}`);
      failed = true;
      continue;
    }

    const source = readFileSync(file.path, "utf8");
    if (check) {
      if (source.includes(MARKER)) {
        console.log(`already patched: ${file.path}`);
        continue;
      }
      const { missing, ambiguous } = file.patch(source);
      console.error(`not patched: ${file.path} (${describeFailure(missing, ambiguous)})`);
      failed = true;
      continue;
    }

    const { code, changed, missing, ambiguous } = file.patch(source);
    if (missing.length > 0 || ambiguous.length > 0) {
      console.error(`cannot patch ${file.path}: ${describeFailure(missing, ambiguous)}`);
      console.error("cargo-dist changed the generated installer; update the anchors.");
      failed = true;
      continue;
    }
    if (!changed) {
      console.log(`already patched: ${file.path}`);
      continue;
    }
    assertSyntax(code, file.path);
    writeFileSync(file.path, code);
    console.log(`patched: ${file.path}`);
  }
  return failed ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main(process.argv);
}
