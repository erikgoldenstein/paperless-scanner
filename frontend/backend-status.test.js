const test = require("node:test");
const assert = require("node:assert/strict");
const { backendWarningText } = require("./backend-status.js");

test("tested backend has no warning", () => {
  assert.equal(backendWarningText({ name: "Linux SANE", experimental: false }), "");
});

test("experimental backend uses the required warning", () => {
  assert.equal(
    backendWarningText({ name: "Windows WIA", experimental: true }),
    "Untested scanner backend (alpha, highly experimental): Windows WIA."
  );
});
