function createDocument(id, pages = []) {
  const copiedPages = [...pages];
  return {
    id,
    pages: copiedPages,
    selected: copiedPages.length ? copiedPages.length - 1 : null,
    upload: null,
    history: []
  };
}

function createArchivedDocument(id, pages = [], identifier = "") {
  return {
    id,
    identifier,
    pages: pages.map(({ preview, thumbnail, rotation }) => ({ preview, thumbnail, rotation })),
    selected: null,
    upload: null,
    history: [],
    archived: true
  };
}

function selectLastPage(document) {
  document.selected = document.pages.length ? document.pages.length - 1 : null;
}

function selectDocument(documents, id) {
  const document = documents.find((candidate) => candidate.id === id);
  if (!document) return null;
  selectLastPage(document);
  return document;
}

function addPage(document, page) {
  const index = document.selected === null ? document.pages.length : document.selected + 1;
  document.pages.splice(index, 0, page);
  document.selected = index;
  return index;
}

function movePage(document, fromIndex, toIndex) {
  if (
    fromIndex < 0 ||
    fromIndex >= document.pages.length ||
    toIndex < 0 ||
    toIndex >= document.pages.length ||
    fromIndex === toIndex
  ) return false;

  const [page] = document.pages.splice(fromIndex, 1);
  document.pages.splice(toIndex, 0, page);
  if (document.selected === fromIndex) document.selected = toIndex;
  else if (document.selected !== null && fromIndex < document.selected && toIndex >= document.selected) document.selected -= 1;
  else if (document.selected !== null && fromIndex > document.selected && toIndex <= document.selected) document.selected += 1;
  return true;
}

function rotatePage(document, index) {
  if (!document || index < 0 || index >= document.pages.length) return null;
  const page = document.pages[index];
  page.rotation = ((Number(page.rotation) || 0) + 90) % 360;
  return page.rotation;
}

function documentsForMode(documents, simpleMode) {
  const currentDocuments = documents.filter((document) => !document.archived);
  if (!simpleMode) return currentDocuments;
  const lastReady = [...currentDocuments].reverse().find((document) => !document.upload);
  return [lastReady || currentDocuments.at(-1)].filter(Boolean);
}

function documentsForBar(documents, simpleMode) {
  const limitedDocuments = limitArchivedDocuments(documents, 25);
  // Archives are retained newest-first, but the tab bar reads left to right.
  // Reverse only the displayed slice so the newest archive is the rightmost.
  const archives = limitedDocuments.filter((document) => document.archived).reverse();
  return [...archives, ...documentsForMode(limitedDocuments, simpleMode)];
}

function limitArchivedDocuments(documents, maximum) {
  const archives = documents.filter((document) => document.archived).slice(0, maximum);
  const currentDocuments = documents.filter((document) => !document.archived);
  return [...archives, ...currentDocuments];
}

function removeDocument(documents, id) {
  return documents.filter((document) => document.id !== id);
}

const documentState = {
  addPage,
  createArchivedDocument,
  createDocument,
  documentsForBar,
  documentsForMode,
  limitArchivedDocuments,
  movePage,
  removeDocument,
  rotatePage,
  selectDocument,
  selectLastPage
};

if (typeof module !== "undefined") module.exports = documentState;
else globalThis.documentState = documentState;
