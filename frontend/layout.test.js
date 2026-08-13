const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");

const stylesheet = fs.readFileSync(`${__dirname}/style.css`, "utf8");

function declarationsFor(selector) {
  const match = stylesheet.match(new RegExp(`\\${selector}\\s*\\{([^}]*)\\}`));
  assert.ok(match, `Expected a CSS rule for ${selector}`);
  return match[1];
}

test("shell regions keep their rows when the backend warning is hidden", () => {
  assert.match(declarationsFor(".backend-warning"), /grid-row:\s*1/);
  assert.match(declarationsFor(".thumbnail-strip"), /grid-row:\s*2/);
  assert.match(declarationsFor(".workspace"), /grid-row:\s*3/);
});

test("the previous archive marker stays vertical beside the tab boundary", () => {
  const declarations = declarationsFor(".archive-hint");
  assert.match(declarations, /transform:\s*translate\(-100%,\s*-50%\)\s*rotate\(-90deg\)/);
  assert.match(declarations, /white-space:\s*nowrap/);
});

test("cancel scan uses the yellow action styling", () => {
  assert.match(declarationsFor(".action.cancel-scan, .primary-button.cancel-scan-confirm"), /background:\s*#d6a11a/);
});
