import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const workflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const config = JSON.parse(readFileSync(new URL("../tauri.conf.json", import.meta.url), "utf8"));
const packageBuilder = readFileSync(new URL("./build-packages.sh", import.meta.url), "utf8");

assert.deepEqual(config.bundle.targets, ["deb", "rpm"]);

for (const runner of ["ubuntu-22.04", "ubuntu-22.04-arm"]) {
  assert.match(workflow, new RegExp(`runner: ${runner.replaceAll(".", "\\.")}`));
}

assert.match(workflow, /bundles: deb,rpm/g);
assert.equal((workflow.match(/bundles: deb,rpm/g) ?? []).length, 2);
assert.match(workflow, /Future release targets; keep disabled/);
assert.match(workflow, /#   bundles: appimage/);
assert.match(workflow, /#   runner: windows-2022/);
assert.match(workflow, /#   runner: macos-15-intel/);
assert.match(workflow, /#   runner: macos-15$/m);
assert.doesNotMatch(workflow, /^\s+runner: windows-2022$/m);
assert.doesNotMatch(workflow, /^\s+runner: macos-15(?:-intel)?$/m);
assert.doesNotMatch(workflow, /^\s+bundles: (?:appimage|msi,nsis|dmg)$/m);

assert.equal((workflow.match(/^\s+artifact:/gm) ?? []).length, 2);
assert.match(workflow, /needs: build/);
assert.match(workflow, /merge-multiple: true/);
assert.match(workflow, /test "\$\{#packages\[@\]\}" -eq 4/);
assert.doesNotMatch(workflow, /^\s+- name: .*AppImage$/m);
assert.doesNotMatch(workflow, /^\s+-name '\*\.AppImage'/m);
assert.doesNotMatch(workflow, /^\s+-name '\*\.msi'/m);
assert.doesNotMatch(workflow, /^\s+-name '\*-setup\.exe'/m);
assert.doesNotMatch(workflow, /^\s+-name '\*\.dmg'/m);
assert.match(workflow, /alpha Linux release/);

assert.match(packageBuilder, /--bundles "\$bundles"/);
assert.match(packageBuilder, /appimage\/\*\.AppImage/);

console.log("release configuration checks passed");
