#!/usr/bin/env node
// Rewrite cargo-dist's generated npm launcher so that a concurrent first
// install cannot destroy another process's install (#504), staging stays on the
// same filesystem as the install directory and every failure path releases the
// lock (#514), downloads retry / fall back to a mirror (#520), and the package
// carries a reachable source list plus per-artifact sha256 (#521).
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
//   node scripts/npm/patch-installer.mjs <package-dir> --checksums-from <dir> [--mirrors <file>|--no-mirrors]

import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  MARKER,
  PATCH_TARGETS,
  assertSyntax,
  describeFailure,
} from "./installer-patches.mjs";
import { MIRRORS_FILENAME, PACKAGE_JSON, injectManifest, verifyManifest } from "./manifest.mjs";

export {
  BINARY_FILENAME,
  INSTALLER_FILENAME,
  MARKER,
  assertSyntax,
  patchBinaryInstall,
  patchBinaryJs,
} from "./installer-patches.mjs";
export {
  ARTIFACT_DIGESTS_KEY,
  MIRRORS_FILENAME,
  collectArtifactDigests,
  expandMirrorUrls,
  injectManifest,
  loadMirrorTemplates,
  verifyManifest,
} from "./manifest.mjs";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));

const USAGE = `usage: node scripts/npm/patch-installer.mjs <package-dir|generated-file> [--check]
       [--checksums-from <dir>] [--mirrors <file>|--no-mirrors]`;

function parseArgs(argv) {
  const args = argv.slice(2);
  // `mirrors`: undefined = default file, null = disabled, string = explicit path.
  const options = { check: false, checksumsFrom: null, mirrors: undefined };
  const targets = [];
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--check") {
      options.check = true;
      continue;
    }
    if (arg === "--no-mirrors") {
      options.mirrors = null;
      continue;
    }
    if (arg === "--checksums-from" || arg === "--mirrors") {
      const value = args[i + 1];
      if (value === undefined || value.startsWith("--")) {
        return { error: `${arg} needs a value` };
      }
      if (arg === "--checksums-from") {
        options.checksumsFrom = value;
      } else {
        options.mirrors = value;
      }
      i += 1;
      continue;
    }
    if (arg.startsWith("-")) {
      return { error: `unknown option: ${arg}` };
    }
    targets.push(arg);
  }
  if (targets.length !== 1) {
    return { error: "exactly one target is required" };
  }
  return { options, target: targets[0] };
}

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

/** Mirror templates path, or null when mirrors are disabled/not applicable. */
function resolveMirrorsPath(target, options) {
  if (options.mirrors === null) {
    return null;
  }
  if (typeof options.mirrors === "string") {
    return options.mirrors;
  }
  if (!existsSync(join(target, PACKAGE_JSON))) {
    return null;
  }
  const defaultPath = join(SCRIPT_DIR, MIRRORS_FILENAME);
  return existsSync(defaultPath) ? defaultPath : null;
}

function patchFiles(files) {
  let failed = false;
  for (const file of files) {
    if (!existsSync(file.path)) {
      console.error(`not found: ${file.path}`);
      failed = true;
      continue;
    }

    const source = readFileSync(file.path, "utf8");
    if (file.check) {
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
  return failed;
}

function main(argv) {
  const parsed = parseArgs(argv);
  if (parsed.error) {
    console.error(parsed.error);
    console.error(USAGE);
    return 2;
  }
  const { options, target } = parsed;

  const isPackageDir = existsSync(target) && statSync(target).isDirectory();
  const wantsManifest = options.checksumsFrom !== null || options.mirrors !== undefined;
  if (wantsManifest && !isPackageDir) {
    console.error("--checksums-from/--mirrors need a package directory");
    console.error(USAGE);
    return 2;
  }

  const files = resolveTargets(target);
  if (files === null) {
    console.error(`unrecognized generated file: ${target}`);
    console.error(USAGE);
    return 2;
  }

  for (const file of files) {
    file.check = options.check;
  }
  let failed = patchFiles(files);

  if (isPackageDir) {
    const mirrorsPath = resolveMirrorsPath(target, options);
    if (!options.check && options.checksumsFrom === null) {
      console.error(
        "warning: no --checksums-from given; the published installer will skip download verification",
      );
    }
    if (mirrorsPath !== null || options.checksumsFrom !== null) {
      const manifestOptions = {
        mirrorsPath: mirrorsPath || undefined,
        checksumsDir: options.checksumsFrom || undefined,
      };
      if (options.check) {
        const failures = verifyManifest(target, manifestOptions);
        if (failures.length > 0) {
          for (const failure of failures) {
            console.error(`manifest: ${failure}`);
          }
          failed = true;
        } else {
          console.log(`manifest ok: ${target}`);
        }
      } else {
        try {
          const result = injectManifest(target, manifestOptions);
          console.log(
            `manifest: ${result.mirrors.length} mirror source(s), ` +
              `${Object.keys(result.digests).length} artifact digest(s)`,
          );
        } catch (e) {
          console.error(`cannot inject manifest: ${e.message}`);
          failed = true;
        }
      }
    }
  }

  return failed ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main(process.argv);
}
