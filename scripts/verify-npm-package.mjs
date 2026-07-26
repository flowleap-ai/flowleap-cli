import { readFile } from "node:fs/promises";

const manifestPath = process.argv[2];

if (!manifestPath) {
  throw new Error(
    "Usage: node scripts/verify-npm-package.mjs <npm-pack-json>",
  );
}

const pack = JSON.parse(await readFile(manifestPath, "utf8"));
const files = pack[0]?.files;

if (!Array.isArray(files)) {
  throw new Error("npm pack output did not contain a files manifest");
}

const actual = files.map(({ path }) => path).sort();
const expected = [
  "LICENSE",
  "README.md",
  "bin/flowleap",
  "download.mjs",
  "package.json",
].sort();

if (JSON.stringify(actual) !== JSON.stringify(expected)) {
  throw new Error(
    `Unexpected npm tarball contents.\nExpected: ${expected.join(", ")}\nActual:   ${actual.join(", ")}`,
  );
}

for (const required of ["README.md", "LICENSE"]) {
  if (!actual.includes(required)) {
    throw new Error(`npm tarball is missing ${required}`);
  }
}

console.log(`Verified npm tarball: ${actual.join(", ")}`);
