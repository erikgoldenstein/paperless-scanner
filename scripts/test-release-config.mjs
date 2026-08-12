import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const workflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const config = JSON.parse(readFileSync(new URL("../tauri.conf.json", import.meta.url), "utf8"));
const packageBuilder = readFileSync(new URL("./build-packages.sh", import.meta.url), "utf8");

assert.equal(config.bundle.targets, "all");
assert.ok(config.bundle.icon.includes("icons/icon.ico"));
assert.ok(config.bundle.icon.includes("icons/icon.icns"));

for (const runner of [
  "ubuntu-22.04",
  "ubuntu-22.04-arm",
  "windows-2022",
  "macos-15-intel",
  "macos-15",
]) {
  assert.match(workflow, new RegExp(`runner: ${runner.replaceAll(".", "\\.")}`));
}

for (const bundle of ["deb,rpm,appimage", "deb,appimage", "msi,nsis", "dmg"]) {
  assert.match(workflow, new RegExp(`bundles: ${bundle}`));
}

assert.equal((workflow.match(/^\s+artifact:/gm) ?? []).length, 5);
assert.match(workflow, /needs: build/);
assert.match(workflow, /merge-multiple: true/);
assert.match(workflow, /APPIMAGE_EXTRACT_AND_RUN: "1"/);
assert.match(workflow, /test "\$\{#packages\[@\]\}" -eq 9/);
assert.match(workflow, /-name '\*\.AppImage'/);
assert.match(workflow, /-name '\*\.msi'/);
assert.match(workflow, /-name '\*\.dmg'/);

assert.match(packageBuilder, /--bundles "\$bundles"/);
assert.match(packageBuilder, /appimage\/\*\.AppImage/);

console.log("release configuration checks passed");
