const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const { detachPagesForUpload, showDialog, closeDialog } = require("./upload-state.js");

test("detaching a document leaves the scan window empty", () => {
  const pages = [{ path: "page-1.png" }, { path: "page-2.png" }];

  const detached = detachPagesForUpload(pages);

  assert.deepEqual(detached.uploadPages, pages);
  assert.deepEqual(detached.currentPages, []);
  assert.equal(detached.selected, null);
  assert.notStrictEqual(detached.uploadPages, pages);
});

test("showDialog makes the custom modal visible", () => {
  let hidden = true;
  let ariaHidden = null;
  const dialog = {
    set hidden(value) { hidden = value; },
    get hidden() { return hidden; },
    setAttribute(name, value) { ariaHidden = [name, value]; }
  };

  showDialog(dialog);

  assert.equal(dialog.hidden, false);
  assert.deepEqual(ariaHidden, ["aria-hidden", "false"]);
});

test("closeDialog hides the custom modal", () => {
  let hidden = false;
  let ariaHidden = null;
  const dialog = {
    hidden,
    setAttribute(name, value) { ariaHidden = [name, value]; }
  };

  closeDialog(dialog);

  assert.equal(dialog.hidden, true);
  assert.deepEqual(ariaHidden, ["aria-hidden", "true"]);
});

test("the app keeps the upload progress UI in the embedded frontend", () => {
  const html = fs.readFileSync(`${__dirname}/index.html`, "utf8");

  assert.match(html, /class="upload-progress"[^>]*role="progressbar"/);
  assert.match(html, /id="upload-progress" class="upload-progress"/);
  assert.match(html, /id="upload-progress-bar"/);
  assert.match(html, /upload-state\.js[\s\S]*app\.js/);
  assert.doesNotMatch(html, /<dialog\b/);
});
