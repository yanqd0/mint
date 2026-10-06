// Release-time injection into the npm package metadata (#521).
//
// The published package must know (a) every reachable download source and (b)
// the sha256 of each release artifact, so an unreachable or tampered mirror
// cannot break or poison an install. cargo-dist only writes the official
// GitHub URL, so the release workflow runs this from
// `patch-installer.mjs` on the unpacked package.

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const PACKAGE_JSON = "package.json";
export const ARTIFACT_DIGESTS_KEY = "artifactSha256";
export const MIRRORS_FILENAME = "mirrors.json";

/**
 * Read the mirror template file.
 *
 * @param {string} path
 * @returns {string[]} templates containing a `{url}` placeholder
 */
export function loadMirrorTemplates(path) {
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  const mirrors = Array.isArray(parsed) ? parsed : parsed && parsed.mirrors;
  if (!Array.isArray(mirrors) || mirrors.length === 0) {
    throw new Error(`no mirrors listed in ${path}`);
  }
  for (const template of mirrors) {
    if (typeof template !== "string" || !template.includes("{url}")) {
      throw new Error(`mirror template needs a {url} placeholder: ${template}`);
    }
  }
  return mirrors;
}

/**
 * Append the expanded mirror URLs after the official ones (idempotent).
 *
 * @param {string[]} officialUrls entries of `artifactDownloadUrls`
 * @param {string[]} templates mirror templates
 * @returns {string[]} the full ordered source list
 */
export function expandMirrorUrls(officialUrls, templates) {
  const urls = officialUrls.filter((url) => typeof url === "string" && url.length > 0);
  // Only official entries are expanded: a URL that already matches a mirror
  // template must not be mirrored again (idempotency, #521).
  const prefixes = templates.map((template) => template.split("{url}")[0]);
  const bases = urls.filter((url) => !prefixes.some((prefix) => url.includes(prefix)));
  for (const template of templates) {
    for (const base of bases) {
      const mirrored = template.replace("{url}", base);
      if (!urls.includes(mirrored)) {
        urls.push(mirrored);
      }
    }
  }
  return urls;
}

/** Every artifact name the package can install, in a stable order. */
export function artifactNames(pkgJson) {
  const platforms = pkgJson.supportedPlatforms || {};
  const names = new Set();
  for (const platform of Object.values(platforms)) {
    if (platform && typeof platform.artifactName === "string") {
      names.add(platform.artifactName);
    }
  }
  return [...names].sort();
}

/** Find `filename` below `dir` (the release steps flatten the artifacts, but be tolerant). */
function findArtifact(dir, filename) {
  const direct = join(dir, filename);
  if (existsSync(direct)) {
    return direct;
  }
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    const nested = findArtifact(join(dir, entry.name), filename);
    if (nested) {
      return nested;
    }
  }
  return null;
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

/**
 * Compute the sha256 of every release artifact the package can install.
 *
 * @param {object} pkgJson
 * @param {string} dir directory holding the release artifacts
 * @returns {Record<string, string>} artifact name -> hex digest
 */
export function collectArtifactDigests(pkgJson, dir) {
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    throw new Error(`not a directory: ${dir}`);
  }
  const digests = {};
  const missing = [];
  for (const name of artifactNames(pkgJson)) {
    const file = findArtifact(dir, name);
    if (file === null) {
      missing.push(name);
      continue;
    }
    digests[name] = sha256(file);
  }
  if (missing.length > 0) {
    throw new Error(
      `release artifacts missing under ${dir}: ${missing.join(", ")}`,
    );
  }
  return digests;
}

/**
 * Inject the mirror sources and the artifact digests into `package.json`.
 *
 * @param {string} pkgDir unpacked npm package
 * @param {{mirrorsPath?: string, checksumsDir?: string}} options
 * @returns {{mirrors: string[], digests: Record<string, string>}}
 */
export function injectManifest(pkgDir, options = {}) {
  const pkgPath = join(pkgDir, PACKAGE_JSON);
  const pkgJson = JSON.parse(readFileSync(pkgPath, "utf8"));
  const official = pkgJson.artifactDownloadUrls || [];
  if (official.length === 0) {
    throw new Error(`${pkgPath} has no artifactDownloadUrls to mirror`);
  }

  const templates = options.mirrorsPath ? loadMirrorTemplates(options.mirrorsPath) : [];
  pkgJson.artifactDownloadUrls = expandMirrorUrls(official, templates);

  const digests = options.checksumsDir
    ? collectArtifactDigests(pkgJson, options.checksumsDir)
    : {};
  if (Object.keys(digests).length > 0) {
    pkgJson[ARTIFACT_DIGESTS_KEY] = { ...digests, ...(pkgJson[ARTIFACT_DIGESTS_KEY] || {}) };
  }

  writeFileSync(pkgPath, `${JSON.stringify(pkgJson, null, 2)}\n`);
  return {
    mirrors: pkgJson.artifactDownloadUrls.slice(official.length),
    digests,
  };
}

/**
 * Verify that an already-published package carries what `injectManifest` writes.
 *
 * @param {string} pkgDir unpacked npm package
 * @param {{mirrorsPath?: string, checksumsDir?: string}} options
 * @returns {string[]} failures (empty when the package is consistent)
 */
export function verifyManifest(pkgDir, options = {}) {
  const failures = [];
  const pkgPath = join(pkgDir, PACKAGE_JSON);
  if (!existsSync(pkgPath)) {
    return [`not found: ${pkgPath}`];
  }
  const pkgJson = JSON.parse(readFileSync(pkgPath, "utf8"));
  const urls = pkgJson.artifactDownloadUrls || [];

  if (options.mirrorsPath) {
    const templates = loadMirrorTemplates(options.mirrorsPath);
    const prefixes = templates.map((template) => template.split("{url}")[0]);
    const official = urls.filter((url) => !prefixes.some((prefix) => url.includes(prefix)));
    for (const base of official) {
      for (const template of templates) {
        const mirrored = template.replace("{url}", base);
        if (!urls.includes(mirrored)) {
          failures.push(`missing download source in ${pkgPath}: ${mirrored}`);
        }
      }
    }
  }

  if (options.checksumsDir) {
    let digests;
    try {
      digests = collectArtifactDigests(pkgJson, options.checksumsDir);
    } catch (e) {
      return [...failures, e.message];
    }
    const recorded = pkgJson[ARTIFACT_DIGESTS_KEY] || {};
    for (const [name, digest] of Object.entries(digests)) {
      if (recorded[name] !== digest) {
        failures.push(
          `sha256 recorded for ${name} does not match the release artifact (${recorded[name] || "absent"})`,
        );
      }
    }
  }

  return failures;
}
