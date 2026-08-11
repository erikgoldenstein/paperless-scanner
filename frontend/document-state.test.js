const test = require("node:test");
const assert = require("node:assert/strict");
const {
  addPage,
  createDocument,
  createArchivedDocument,
  documentsForBar,
  documentsForMode,
  limitArchivedDocuments,
  movePage,
  removeDocument,
  rotatePage,
  selectDocument,
  selectLastPage
} = require("./document-state.js");

test("adding a page inserts it after the selected tab", () => {
  const document = createDocument("document-1", ["page-1", "page-2", "page-3"]);
  document.selected = 0;

  addPage(document, "page-new");

  assert.deepEqual(document.pages, ["page-1", "page-new", "page-2", "page-3"]);
  assert.equal(document.selected, 1);
});

test("switching to a document selects its last page", () => {
  const documents = [
    createDocument("document-1", ["page-1"]),
    createDocument("document-2", ["page-2", "page-3"])
  ];
  documents[1].selected = 0;

  selectDocument(documents, "document-2");

  assert.equal(documents[1].selected, 1);
});

test("simple mode exposes only the last non-uploading document", () => {
  const documents = [
    createDocument("document-1", ["page-1"]),
    { ...createDocument("document-2", ["page-2"]), upload: { state: "active" } },
    createDocument("document-3", ["page-3"])
  ];

  assert.deepEqual(documentsForMode(documents, true), [documents[2]]);
});

test("simple mode keeps one active document while older groups upload", () => {
  const documents = [
    { ...createDocument("document-1", ["page-1"]), upload: { state: "active" } },
    { ...createDocument("document-2", ["page-2"]), upload: { state: "active" } },
    createDocument("document-3", ["page-3"])
  ];

  assert.deepEqual(documentsForMode(documents, true), [documents[2]]);
});

test("completed upload groups are removed from the tab bar", () => {
  const documents = [
    createDocument("document-1", ["page-1"]),
    createDocument("document-2", ["page-2"])
  ];

  const remaining = removeDocument(documents, "document-1");

  assert.deepEqual(remaining.map(({ id }) => id), ["document-2"]);
});

test("an empty document selects no page until the first scan", () => {
  const document = createDocument("document-1");

  selectLastPage(document);

  assert.equal(document.selected, null);
});

test("moving a page updates the order and keeps it selected", () => {
  const document = createDocument("document-1", ["page-1", "page-2", "page-3"]);
  document.selected = 0;

  movePage(document, 0, 2);

  assert.deepEqual(document.pages, ["page-2", "page-3", "page-1"]);
  assert.equal(document.selected, 2);
});

test("archived documents keep preview data but no filesystem paths", () => {
  const archived = createArchivedDocument("archive-upload-1", [{
    path: "/tmp/page-1.png",
    preview: "preview-1",
    thumbnail: "thumbnail-1",
    rotation: 90
  }], "aB12cD34");

  assert.equal(archived.archived, true);
  assert.equal(archived.identifier, "aB12cD34");
  assert.equal(archived.selected, null);
  assert.deepEqual(archived.pages, [{
    preview: "preview-1",
    thumbnail: "thumbnail-1",
    rotation: 90
  }]);
});

test("archive groups are ordered before current groups and excluded from simple mode selection", () => {
  const archive = createArchivedDocument("archive-upload-1", []);
  const current = createDocument("document-2", ["page-2"]);
  const otherCurrent = createDocument("document-3", ["page-3"]);

  assert.deepEqual(documentsForBar([current, archive, otherCurrent], false), [archive, current, otherCurrent]);
  assert.deepEqual(documentsForBar([current, archive, otherCurrent], true), [archive, otherCurrent]);
});

test("only the newest 25 archived documents are retained", () => {
  const archives = Array.from({ length: 26 }, (_, index) =>
    createArchivedDocument(`archive-${25 - index}`, [])
  );
  const current = createDocument("document-current", ["page-current"]);

  const limited = limitArchivedDocuments([...archives, current], 25);

  assert.equal(limited.filter((document) => document.archived).length, 25);
  assert.equal(limited[0].id, "archive-25");
  assert.equal(limited.at(-1).id, "document-current");
});

test("page rotation remains attached to the page after selecting another page", () => {
  const document = createDocument("document-1", [
    { path: "page-1", rotation: 0 },
    { path: "page-2", rotation: 0 }
  ]);

  rotatePage(document, 1);
  selectDocument([document], document.id);
  document.selected = 0;
  document.selected = 1;

  assert.equal(document.pages[1].rotation, 90);
});
