const invoke = (command, args) => window.__TAURI__.core.invoke(command, args);

const documentStateApi = window.documentState;
const insertPage = documentStateApi.addPage;
const createArchivedDocumentState = documentStateApi.createArchivedDocument;
const createDocumentState = documentStateApi.createDocument;
const getDocumentsForBar = documentStateApi.documentsForBar;
const getDocumentsForMode = documentStateApi.documentsForMode;
const limitArchivedDocumentsState = documentStateApi.limitArchivedDocuments;
const removeDocumentState = documentStateApi.removeDocument;
const rotatePageState = documentStateApi.rotatePage;
const selectDocumentState = documentStateApi.selectDocument;
const selectLastPageState = documentStateApi.selectLastPage;

const state = {
  documents: [createDocumentState("document-1")],
  activeDocumentId: "document-1",
  scanDocumentId: "document-1",
  nextDocumentNumber: 2,
  nextJobNumber: 1,
  settings: {
    scanner: { device: "", resolution: 300, mode: "Color" },
    paperless_url: "",
    paperless_token: "",
    compression: 85,
    compression_format: "jpeg",
    paper_format: "a4",
    max_upload_size_mb: 10,
    simple_mode: false,
    ask_for_filename: true,
    hash_file_naming: true,
    debug_history: false,
    theme: "system"
  },
  busy: false,
  scanning: false,
  scanGeneration: 0,
  uploadDialogJob: null,
  uploadDialogDocumentId: null,
  confirmation: null
};

let pageDrag = null;
let suppressPageClick = false;
let tabBarSnapTimer = null;
const previewZoom = {
  scale: 1,
  offsetX: 0,
  offsetY: 0,
  pointers: new Map(),
  drag: null,
  pinch: null
};

const MIN_PREVIEW_ZOOM = 1;
const MAX_PREVIEW_ZOOM = 4;

const $ = (id) => document.getElementById(id);

const systemThemeQuery = window.matchMedia?.("(prefers-color-scheme: dark)") || null;

function normalizedTheme(theme) {
  return ["dark", "light", "system"].includes(theme) ? theme : "system";
}

function applyTheme(theme = state.settings.theme) {
  const selectedTheme = normalizedTheme(theme);
  const dark = selectedTheme === "dark" || (selectedTheme === "system" && systemThemeQuery?.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

function watchSystemTheme() {
  if (!systemThemeQuery) return;
  const update = () => {
    if (normalizedTheme(state.settings.theme) === "system") applyTheme();
  };
  if (systemThemeQuery.addEventListener) systemThemeQuery.addEventListener("change", update);
  else systemThemeQuery.addListener?.(update);
}

applyTheme();
watchSystemTheme();

function setStatus(message = "", kind = "") {
  const status = $("status");
  status.textContent = message;
  status.className = `status ${kind}`;
}

function recordHistory(target, message, kind = "debug") {
  if (!target) return;
  target.history ||= [];
  if (target.history.at(-1)?.message === message) return;
  target.history.push({ message, kind, time: new Date().toLocaleTimeString() });
}

function historyFor(document) {
  return groupJob(document)?.history || document?.history || [];
}

function visibleHistory(entries) {
  return state.settings.debug_history ? entries : entries.filter((entry) => entry.kind === "update");
}

function openStatusDialog() {
  const list = $("status-history-list");
  list.replaceChildren();
  const seenJobs = new Set();
  let shown = 0;
  state.documents.forEach((group) => {
    const job = groupJob(group);
    if (job && seenJobs.has(job.id)) return;
    if (job) seenJobs.add(job.id);
    const entries = visibleHistory(historyFor(group));
    if (!entries.length) return;
    shown += 1;
    const details = globalThis.document.createElement("details");
    details.open = group.id === state.activeDocumentId;
    const summary = globalThis.document.createElement("summary");
    summary.textContent = documentLabel(group);
    details.append(summary);
    const historyList = globalThis.document.createElement("ul");
    entries.forEach((entry) => {
      const item = globalThis.document.createElement("li");
      item.textContent = `${entry.time} — ${entry.message}`;
      historyList.append(item);
    });
    details.append(historyList);
    $("status-history-list").append(details);
  });
  if (!shown) {
    const empty = globalThis.document.createElement("p");
    empty.textContent = state.settings.debug_history
      ? "No state history yet."
      : "No upload/update history yet. Enable debug details to include scan actions.";
    list.append(empty);
  }
  showDialog($("status-dialog"));
}

function documentById(id) {
  return state.documents.find((document) => document.id === id) || null;
}

function currentDocument() {
  if (state.settings.simple_mode) {
    const selected = documentById(state.activeDocumentId);
    if (selected?.archived) return selected;
    return getDocumentsForMode(state.documents, true)[0] || null;
  }
  return documentById(state.activeDocumentId);
}

function pageLabel(index) {
  return `Page ${index + 1}`;
}

function pageRotation(page) {
  return ((Number(page?.rotation) || 0) % 360 + 360) % 360;
}

function applyPageRotation(image, page) {
  const rotation = pageRotation(page);
  image.style.transform = rotation ? `rotate(${rotation}deg)` : "";
  image.style.transformOrigin = "center";
}

function previewImageSize(image, page) {
  const quarterTurn = pageRotation(page) % 180 !== 0;
  return {
    width: quarterTurn ? image.offsetHeight : image.offsetWidth,
    height: quarterTurn ? image.offsetWidth : image.offsetHeight
  };
}

function clampPreviewOffset() {
  const image = $("preview")?.querySelector("img");
  const group = currentDocument();
  const page = group?.selected === null ? null : group?.pages[group?.selected];
  if (!image || !page) {
    previewZoom.offsetX = 0;
    previewZoom.offsetY = 0;
    return;
  }
  const size = previewImageSize(image, page);
  const preview = $("preview");
  const maxX = Math.max(0, (size.width * previewZoom.scale - preview.clientWidth) / 2);
  const maxY = Math.max(0, (size.height * previewZoom.scale - preview.clientHeight) / 2);
  previewZoom.offsetX = Math.max(-maxX, Math.min(maxX, previewZoom.offsetX));
  previewZoom.offsetY = Math.max(-maxY, Math.min(maxY, previewZoom.offsetY));
}

function applyPreviewTransform(image, page) {
  const rotation = pageRotation(page);
  image.style.transform = `translate(${previewZoom.offsetX}px, ${previewZoom.offsetY}px) scale(${previewZoom.scale}) rotate(${rotation}deg)`;
  image.style.transformOrigin = "center";
}

function updateZoomControls() {
  const hasImage = Boolean($("preview")?.querySelector("img"));
  $("preview")?.classList.toggle("zoomed", hasImage && previewZoom.scale > MIN_PREVIEW_ZOOM);
  $("zoom-out").disabled = !hasImage || previewZoom.scale <= MIN_PREVIEW_ZOOM;
  $("zoom-in").disabled = !hasImage || previewZoom.scale >= MAX_PREVIEW_ZOOM;
  $("zoom-level").textContent = `${Math.round(previewZoom.scale * 100)}%`;
}

function resetPreviewZoom() {
  previewZoom.scale = MIN_PREVIEW_ZOOM;
  previewZoom.offsetX = 0;
  previewZoom.offsetY = 0;
  previewZoom.pointers.clear();
  previewZoom.drag = null;
  previewZoom.pinch = null;
  updateZoomControls();
}

function previewPoint(event) {
  const bounds = $("preview").getBoundingClientRect();
  return {
    x: event.clientX - bounds.left - bounds.width / 2,
    y: event.clientY - bounds.top - bounds.height / 2
  };
}

function setPreviewZoom(scale, anchor = { x: 0, y: 0 }) {
  const nextScale = Math.max(MIN_PREVIEW_ZOOM, Math.min(MAX_PREVIEW_ZOOM, scale));
  const oldScale = previewZoom.scale;
  if (nextScale === oldScale) return;
  previewZoom.offsetX = anchor.x - ((anchor.x - previewZoom.offsetX) * nextScale) / oldScale;
  previewZoom.offsetY = anchor.y - ((anchor.y - previewZoom.offsetY) * nextScale) / oldScale;
  previewZoom.scale = nextScale;
  clampPreviewOffset();
  const group = currentDocument();
  const page = group?.selected === null ? null : group?.pages[group?.selected];
  const image = $("preview")?.querySelector("img");
  if (image && page) applyPreviewTransform(image, page);
  updateZoomControls();
}

function changePreviewZoom(direction, anchor) {
  setPreviewZoom(previewZoom.scale + direction * 0.25, anchor);
}

function previewPointerDown(event) {
  if (!$("preview").querySelector("img")) return;
  try {
    $("preview").setPointerCapture?.(event.pointerId);
  } catch {
    // Synthetic pointer events and some embedded webviews do not support capture.
  }
  previewZoom.pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  if (previewZoom.pointers.size === 1) {
    previewZoom.drag = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      offsetX: previewZoom.offsetX,
      offsetY: previewZoom.offsetY
    };
  } else if (previewZoom.pointers.size === 2) {
    const points = [...previewZoom.pointers.values()];
    previewZoom.drag = null;
    previewZoom.pinch = {
      distance: Math.max(1, Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y)),
      scale: previewZoom.scale
    };
  }
}

function previewPointerMove(event) {
  if (!previewZoom.pointers.has(event.pointerId)) return;
  previewZoom.pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  if (previewZoom.pointers.size >= 2 && previewZoom.pinch) {
    const points = [...previewZoom.pointers.values()];
    const distance = Math.max(1, Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y));
    const midpoint = {
      clientX: (points[0].x + points[1].x) / 2,
      clientY: (points[0].y + points[1].y) / 2
    };
    setPreviewZoom(previewZoom.pinch.scale * distance / previewZoom.pinch.distance, previewPoint(midpoint));
    event.preventDefault();
    return;
  }
  const drag = previewZoom.drag;
  if (!drag || drag.pointerId !== event.pointerId || previewZoom.scale <= MIN_PREVIEW_ZOOM) return;
  previewZoom.offsetX = drag.offsetX + event.clientX - drag.startX;
  previewZoom.offsetY = drag.offsetY + event.clientY - drag.startY;
  clampPreviewOffset();
  const group = currentDocument();
  const page = group?.selected === null ? null : group?.pages[group.selected];
  const image = $("preview").querySelector("img");
  if (image && page) applyPreviewTransform(image, page);
  $("preview").classList.add("panning");
  event.preventDefault();
}

function previewPointerUp(event) {
  previewZoom.pointers.delete(event.pointerId);
  if (previewZoom.pointers.size < 2) previewZoom.pinch = null;
  if (previewZoom.pointers.size === 1) {
    const [pointerId, point] = previewZoom.pointers.entries().next().value;
    previewZoom.drag = {
      pointerId,
      startX: point.x,
      startY: point.y,
      offsetX: previewZoom.offsetX,
      offsetY: previewZoom.offsetY
    };
  } else {
    previewZoom.drag = null;
  }
  $("preview").classList.remove("panning");
}

function previewWheel(event) {
  if (!$("preview").querySelector("img")) return;
  event.preventDefault();
  changePreviewZoom(event.deltaY < 0 ? 1 : -1, previewPoint(event));
}

function fitPreviewImage(image, page) {
  resetPreviewZoom();
  applyPreviewTransform(image, page);
  const fit = () => {
    if (!image.naturalWidth || !image.naturalHeight) return;
    const preview = $("preview");
    const quarterTurn = pageRotation(page) % 180 !== 0;
    const sourceWidth = quarterTurn ? image.naturalHeight : image.naturalWidth;
    const sourceHeight = quarterTurn ? image.naturalWidth : image.naturalHeight;
    const scale = Math.min(preview.clientWidth / sourceWidth, preview.clientHeight / sourceHeight, 1);
    image.style.maxWidth = "none";
    image.style.maxHeight = "none";
    image.style.width = `${Math.max(1, Math.round(image.naturalWidth * scale))}px`;
    image.style.height = `${Math.max(1, Math.round(image.naturalHeight * scale))}px`;
    applyPreviewTransform(image, page);
  };
  image.addEventListener("load", fit, { once: true });
  requestAnimationFrame(fit);
}

function documentLabel(document) {
  return `Document ${document.id.replace("document-", "")}`;
}

function groupJob(document) {
  return document.upload || document.backgroundJob || null;
}

function updateActionButtons() {
  const document = currentDocument();
  const hasPages = Boolean(document?.pages?.length);
  const scannerLocked = state.scanning || document?.upload?.state === "active";
  $("add-page").disabled = scannerLocked || !document;
  $("rescan").disabled = scannerLocked || !document || !hasPages;
  $("upload").disabled = state.busy || !document || !hasPages;
  $("reset").disabled = state.busy || !document || !hasPages;
  $("rotate-preview").disabled = state.busy || !document || document.selected === null;
  $("settings-button").disabled = state.busy;
}

function renderDocumentGroups(options = {}) {
  const container = $("document-groups");
  const previousScrollLeft = container.scrollLeft;
  const previousMaximumScroll = Math.max(0, container.scrollWidth - container.clientWidth);
  const wasAtRightEdge = previousScrollLeft >= previousMaximumScroll - 2;
  container.replaceChildren();
  const simpleMode = state.settings.simple_mode;
  const documents = getDocumentsForBar(state.documents, simpleMode);

  if (documents.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty-message";
    empty.textContent = "No documents yet. Add a page to start.";
    container.append(empty);
    requestAnimationFrame(() => { container.scrollLeft = 0; });
    return;
  }

  documents.forEach((group) => {
    const section = document.createElement("section");
    const classes = ["document-group"];
    const readOnly = simpleMode || group.archived;
    if (group.id === state.activeDocumentId || (simpleMode && !group.archived)) classes.push("active");
    if (group.upload?.state === "active") classes.push("uploading");
    if (group.upload?.state === "failed") classes.push("upload-failed");
    if (group.archived) classes.push("archived");
    section.className = classes.join(" ");
    section.dataset.documentId = group.id;
    if (!readOnly) {
      section.addEventListener("click", (event) => {
        if (event.target.closest(".thumb-remove")) return;
        if (group.id !== state.activeDocumentId) switchDocument(group.id);
      });
    }

    if (group.archived || !readOnly) {
      const header = document.createElement("div");
      header.className = "document-group-header";
      const switcher = document.createElement("button");
      switcher.className = "document-tab";
      if (group.archived) switcher.classList.add("archive-identifier");
      switcher.type = "button";
      switcher.textContent = group.archived ? (group.identifier || "Archived document") : documentLabel(group);
      switcher.title = group.archived ? "Show this archived document" : `Switch to ${documentLabel(group)}`;
      switcher.addEventListener("click", () => switchDocument(group.id));
      header.append(switcher);
      if (!group.archived) {
        const count = document.createElement("span");
        count.className = "document-count";
        count.textContent = `${group.pages.length} page${group.pages.length === 1 ? "" : "s"}`;
        header.append(count);
        if (group.upload?.state === "active") {
          const badge = document.createElement("span");
          badge.className = "upload-badge";
          badge.textContent = "Uploading";
          header.append(badge);
        }
      }
      section.append(header);
    }

    const tabs = document.createElement("div");
    tabs.className = "page-tabs";
    if (group.pages.length === 0) {
      const empty = document.createElement("span");
      empty.className = "empty-message";
      empty.textContent = "No pages yet. Add a page to start.";
      tabs.append(empty);
    } else {
      group.pages.forEach((page, index) => {
        const frame = document.createElement("div");
        frame.className = "thumbnail-frame";
        frame.dataset.documentId = group.id;
        frame.dataset.pageIndex = String(index);
        frame.dataset.pageKey = page.path || "";
        if (!readOnly) frame.addEventListener("pointerdown", (event) => beginPageDrag(event, group.id, frame));
        const tab = document.createElement("button");
        tab.className = `thumbnail ${index === group.selected ? "selected" : ""}`;
        tab.type = "button";
        tab.title = `${pageLabel(index)} in ${documentLabel(group)}`;
        tab.disabled = !group.archived && readOnly;
        if (group.archived || !readOnly) tab.addEventListener("click", (event) => {
          if (suppressPageClick) {
            suppressPageClick = false;
            event.preventDefault();
            return;
          }
          selectPage(group.id, Number(frame.dataset.pageIndex));
        });
        const image = document.createElement("img");
        image.src = page.thumbnail || page.preview;
        image.alt = pageLabel(index);
        applyPageRotation(image, page);
        tab.append(image);
        frame.append(tab);

        if (!readOnly) {
          const remove = document.createElement("button");
          remove.className = "thumb-remove";
          remove.type = "button";
          remove.title = `Remove ${pageLabel(index)}`;
          remove.setAttribute("aria-label", `Remove ${pageLabel(index)}`);
          remove.textContent = "×";
          remove.addEventListener("pointerdown", (event) => event.stopPropagation());
          remove.addEventListener("click", (event) => {
            event.stopPropagation();
            openConfirmDialog(
              "Remove this page?",
              `The ${pageLabel(index).toLowerCase()} will be permanently removed from this document.`,
              () => { void removePage(group.id, Number(frame.dataset.pageIndex), page.path); }
            );
          });
          frame.append(remove);
        }
        tabs.append(frame);
      });
    }
    section.append(tabs);
    container.append(section);
  });

  const archivedSections = [...container.querySelectorAll(".document-group.archived")];
  const currentSections = [...container.querySelectorAll(".document-group:not(.archived)")];
  if (archivedSections.length && currentSections.length) {
    const currentBoundary = currentSections[0];
    currentBoundary.classList.add("current-boundary");
    const archiveBoundaryFade = document.createElement("div");
    archiveBoundaryFade.className = "archive-boundary-fade";
    const archiveHint = document.createElement("span");
    archiveHint.className = "archive-hint";
    archiveHint.textContent = "previous";
    archiveHint.setAttribute("aria-hidden", "true");
    container.append(archiveBoundaryFade, archiveHint);
    const containerBox = container.getBoundingClientRect();
    const currentBox = currentBoundary.getBoundingClientRect();
    const boundaryLeft = currentBox.left - containerBox.left + container.scrollLeft;
    archiveBoundaryFade.style.left = `${boundaryLeft}px`;
    archiveHint.style.left = `${boundaryLeft}px`;
    const gap = Number.parseFloat(getComputedStyle(container).columnGap || getComputedStyle(container).gap) || 0;
    const leadingMargin = Number.parseFloat(getComputedStyle(currentBoundary).marginLeft) || 0;
    const currentWidth = currentSections.reduce((total, section) => total + section.getBoundingClientRect().width, 0)
      + gap * Math.max(0, currentSections.length - 1)
      + leadingMargin;
    if (currentWidth < container.clientWidth) {
      const spacer = document.createElement("div");
      spacer.className = "archive-spacer";
      spacer.style.width = `${Math.max(0, container.clientWidth - currentWidth - gap)}px`;
      container.append(spacer);
    }
  }

  requestAnimationFrame(() => {
    const maximumScroll = Math.max(0, container.scrollWidth - container.clientWidth);
    container.scrollLeft = options.scrollToEnd || wasAtRightEdge
      ? maximumScroll
      : Math.min(previousScrollLeft, maximumScroll);
  });
}

function archiveBoundaryScrollTarget(container) {
  if (!container.querySelector(".document-group.archived")) return null;
  const current = container.querySelector(".document-group:not(.archived)");
  if (!current) return null;
  const containerBox = container.getBoundingClientRect();
  const currentBox = current.getBoundingClientRect();
  const contentLeft = currentBox.left - containerBox.left + container.scrollLeft;
  return Math.max(0, Math.min(container.scrollWidth - container.clientWidth, contentLeft));
}

function scheduleArchiveBoundarySnap() {
  const container = $("document-groups");
  window.clearTimeout(tabBarSnapTimer);
  tabBarSnapTimer = window.setTimeout(() => {
    const target = archiveBoundaryScrollTarget(container);
    if (target === null || Math.abs(container.scrollLeft - target) > 80) return;
    container.scrollTo({ left: target, behavior: "smooth" });
  }, 120);
}

function renderPreview() {
  const preview = $("preview");
  const group = currentDocument();
  resetPreviewZoom();
  preview.replaceChildren();
  if (group?.selected !== null && group?.pages[group.selected]) {
    const image = document.createElement("img");
    image.src = group.pages[group.selected].preview;
    image.alt = `Current ${pageLabel(group.selected).toLowerCase()}`;
    fitPreviewImage(image, group.pages[group.selected]);
    preview.append(image);
    preview.className = "preview-area";
  } else {
    const message = document.createElement("span");
    message.textContent = "Your current page will appear here";
    preview.append(message);
    preview.className = "preview-area preview-empty";
  }
  updateZoomControls();
}

function renderUploadStatus() {
  const status = $("upload-status");
  const indicator = $("upload-indicator");
  const progress = $("upload-progress");
  const progressBar = $("upload-progress-bar");
  const text = $("upload-status-text");
  const document = currentDocument();
  const job = groupJob(document);
  if (!job) {
    status.hidden = false;
    status.className = "upload-status idle";
    indicator.hidden = true;
    progress.hidden = true;
    text.textContent = "";
    status.onclick = openStatusDialog;
    return;
  }

  status.hidden = false;
  status.className = "upload-status";
  indicator.hidden = false;
  progress.hidden = false;
  indicator.className = `upload-indicator ${job.state}`;
  indicator.disabled = job.state === "active";
  indicator.onclick = job.state === "failed" ? (event) => {
    event.stopPropagation();
    if (state.settings.ask_for_filename) {
      openUploadDialog(job);
    } else {
      startUpload(documentById(job.documentId), job.title, job);
    }
  } : null;
  status.onclick = openStatusDialog;
  progressBar.className = `upload-progress-bar ${job.state}`;
  progress.setAttribute("aria-busy", job.state === "active" ? "true" : "false");
  progressBar.setAttribute("aria-valuetext", job.progress);
  text.textContent = job.state === "failed"
    ? `Upload failed: ${job.error} (click to retry)`
    : job.progress;
}

function render(options = {}) {
  renderDocumentGroups(options);
  renderPreview();
  renderUploadStatus();
  updateActionButtons();
}

function refreshVisibleSelection() {
  const active = currentDocument();
  document.querySelectorAll(".document-group").forEach((section) => {
    const group = documentById(section.dataset.documentId);
    if (!group) return;
    section.classList.toggle("active", group.id === state.activeDocumentId || state.settings.simple_mode);
    section.querySelectorAll(".thumbnail").forEach((tab, index) => {
      tab.classList.toggle("selected", index === group.selected);
      const image = tab.querySelector("img");
      if (image && group.pages[index]) applyPageRotation(image, group.pages[index]);
    });
  });
  renderPreview();
  renderUploadStatus();
  updateActionButtons();
  return active;
}

function setBusy(busy) {
  state.busy = busy;
  updateActionButtons();
}

function beginPageDrag(event, documentId, frame) {
  if (event.button !== 0) return;
  frame.setPointerCapture?.(event.pointerId);
  const initialFrames = [...frame.parentElement.querySelectorAll(".thumbnail-frame")]
    .filter((candidate) => candidate !== frame)
    .map((candidate) => ({
      frame: candidate,
      center: candidate.getBoundingClientRect().left + candidate.getBoundingClientRect().width * 0.4
    }));
  pageDrag = {
    documentId,
    fromIndex: Number(frame.dataset.pageIndex),
    frame,
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    moved: false,
    initialFrames
  };
}

function movePageTabs(documentId, fromIndex, toIndex) {
  const section = [...document.querySelectorAll(".document-group")]
    .find((candidate) => candidate.dataset.documentId === documentId);
  const tabs = section?.querySelector(".page-tabs");
  if (!tabs) return;
  const frames = [...tabs.querySelectorAll(".thumbnail-frame")];
  const before = new Map(frames.map((frame) => [frame, frame.getBoundingClientRect()]));
  const frame = frames[fromIndex];
  const target = frames[toIndex];
  if (!frame || !target || frame === target) return;
  if (fromIndex < toIndex) tabs.insertBefore(frame, target.nextSibling);
  else tabs.insertBefore(frame, target);

  [...tabs.querySelectorAll(".thumbnail-frame")].forEach((current, index) => {
    current.dataset.pageIndex = String(index);
    const tab = current.querySelector(".thumbnail");
    const remove = current.querySelector(".thumb-remove");
    if (tab) tab.title = `${pageLabel(index)} in ${documentLabel(documentById(documentId))}`;
    if (remove) {
      remove.title = `Remove ${pageLabel(index)}`;
      remove.setAttribute("aria-label", `Remove ${pageLabel(index)}`);
    }
  });

  frames.forEach((current) => {
    if (current === frame) return;
    const oldBox = before.get(current);
    const newBox = current.getBoundingClientRect();
    const offset = oldBox ? oldBox.left - newBox.left : 0;
    if (!offset) return;
    current.classList.add("tab-moving");
    current.style.transition = "none";
    current.style.transform = `translateX(${offset}px)`;
    requestAnimationFrame(() => {
      current.style.transition = "";
      current.style.transform = "";
    });
    window.setTimeout(() => current.classList.remove("tab-moving"), 160);
  });
}

function pageDragMove(event) {
  if (!pageDrag || event.pointerId !== pageDrag.pointerId) return;
  const distance = Math.hypot(event.clientX - pageDrag.startX, event.clientY - pageDrag.startY);
  if (!pageDrag.moved && distance < 6) return;
  pageDrag.moved = true;
  event.preventDefault();
  pageDrag.frame.classList.add("dragging");
  document.querySelectorAll(".thumbnail-frame.drag-over").forEach((target) => target.classList.remove("drag-over"));
  const section = [...document.querySelectorAll(".document-group")]
    .find((candidate) => candidate.dataset.documentId === pageDrag.documentId);
  const tabs = section?.querySelector(".page-tabs");
  const frames = tabs ? [...tabs.querySelectorAll(".thumbnail-frame")] : [];
  if (!pageDrag.initialFrames.length) return;
  const targetIndex = pageDrag.initialFrames.findIndex(({ center }) => event.clientX < center);
  const target = pageDrag.initialFrames[targetIndex === -1 ? pageDrag.initialFrames.length - 1 : targetIndex].frame;
  target.classList.add("drag-over");
  const fromIndex = frames.indexOf(pageDrag.frame);
  const desiredIndex = targetIndex === -1 ? pageDrag.initialFrames.length : targetIndex;
  if (fromIndex !== desiredIndex) {
    movePageTabs(pageDrag.documentId, fromIndex, desiredIndex);
  }
}

function pageDragEnd(event) {
  if (!pageDrag || event.pointerId !== pageDrag.pointerId) return;
  const drag = pageDrag;
  pageDrag = null;
  drag.frame.classList.remove("dragging");
  document.querySelectorAll(".thumbnail-frame.drag-over").forEach((target) => target.classList.remove("drag-over"));
  if (!drag.moved) {
    const group = documentById(drag.documentId);
    if (group) {
      group.selected = drag.fromIndex;
      state.activeDocumentId = group.id;
      if (!group.archived) state.scanDocumentId = group.id;
      refreshVisibleSelection();
    }
    return;
  }
  if (event.type === "pointercancel") {
    renderDocumentGroups();
    refreshVisibleSelection();
    return;
  }
  suppressPageClick = true;
  const group = documentById(drag.documentId);
  const section = [...document.querySelectorAll(".document-group")]
    .find((candidate) => candidate.dataset.documentId === drag.documentId);
  const frames = section ? [...section.querySelectorAll(".thumbnail-frame")] : [];
  const selectedPage = group?.selected === null ? null : group?.pages[group.selected];
  const orderedPages = frames
    .map((frame) => group?.pages.find((page) => page.path === frame.dataset.pageKey))
    .filter(Boolean);
  if (group && orderedPages.length === group.pages.length) {
    group.pages = orderedPages;
    group.selected = selectedPage ? group.pages.indexOf(selectedPage) : null;
    state.activeDocumentId = group.id;
    if (!group.archived) state.scanDocumentId = group.id;
    recordHistory(group, "Page order changed");
    refreshVisibleSelection();
  }
}

document.addEventListener("pointermove", pageDragMove);
document.addEventListener("pointerup", pageDragEnd);
document.addEventListener("pointercancel", pageDragEnd);

function switchDocument(id) {
  const target = documentById(id);
  if (state.settings.simple_mode && !target?.archived) return;
  const selected = selectDocumentState(state.documents, id);
  if (!selected) return;
  state.activeDocumentId = id;
  if (!selected.archived) state.scanDocumentId = id;
  refreshVisibleSelection();
  setStatus(`${documentLabel(selected)} selected.`, "success");
}

function jumpToScanDocument() {
  let target = documentById(state.scanDocumentId);
  if (!target || target.archived) target = [...state.documents].reverse().find((document) => !document.archived);
  if (!target) return null;
  state.scanDocumentId = target.id;
  if (state.activeDocumentId !== target.id) {
    selectLastPageState(target);
    state.activeDocumentId = target.id;
    refreshVisibleSelection();
  }
  return target;
}

function selectPage(documentId, index) {
  const group = documentById(documentId);
  if (!group || index < 0 || index >= group.pages.length) return;
  group.selected = index;
  state.activeDocumentId = group.id;
  if (!group.archived) state.scanDocumentId = group.id;
  refreshVisibleSelection();
}

async function addPage() {
  jumpToScanDocument();
  await scan(false);
}

function openConfirmDialog(title, message, onConfirm) {
  state.confirmation = onConfirm;
  $("confirm-title").textContent = title;
  $("confirm-message").textContent = message;
  showDialog($("confirm-dialog"));
}

function closeConfirmDialog() {
  state.confirmation = null;
  closeDialog($("confirm-dialog"));
}

function rescan() {
  jumpToScanDocument();
  const document = currentDocument();
  if (document?.selected === null || !document) {
    setStatus("Select a page to rescan.", "error");
    return;
  }
  openConfirmDialog(
    "Rescan this page?",
    "The current page will be replaced by the new scan.",
    () => { void scan(true); }
  );
}

async function scan(replace) {
  const document = currentDocument();
  if (!document || state.scanning) return;
  const scanGeneration = ++state.scanGeneration;
  state.scanning = true;
  updateActionButtons();
  $("scanner-status").textContent = "Scanning…";
  recordHistory(document, replace ? "Rescan started" : "Scan started");
  setStatus("Scanning…");
  try {
    const page = await invoke("scan_page", { settings: state.settings.scanner });
    if (scanGeneration !== state.scanGeneration) {
      await cleanup([page.path]);
      return;
    }
    if (replace) {
      const oldPage = document.pages[document.selected];
      document.pages[document.selected] = page;
      await cleanup([oldPage.path]);
      recordHistory(document, "Page rescanned");
    } else {
      insertPage(document, page);
      recordHistory(document, "Page added");
    }
    render();
    setStatus(`${document.pages.length} page${document.pages.length === 1 ? "" : "s"} ready.`, "success");
    $("scanner-status").textContent = "Scanner ready.";
  } catch (error) {
    const message = String(error);
    recordHistory(document, `Scan failed: ${message}`);
    setStatus(message, "error");
    $("scanner-status").textContent = message;
  } finally {
    state.scanning = false;
    updateActionButtons();
  }
}

async function cleanup(paths) {
  if (paths.length) await invoke("cleanup_pages", { paths });
}

function reset() {
  jumpToScanDocument();
  const document = currentDocument();
  if (!document || document.pages.length === 0) return;
  openConfirmDialog(
    "Reset this document?",
    "All scanned pages in the current document will be deleted.",
    () => { void clearCurrentDocument(); }
  );
}

async function clearCurrentDocument() {
  const document = currentDocument();
  if (!document || document.pages.length === 0) return;
  state.scanGeneration += 1;
  recordHistory(document, "Reset started");
  setBusy(true);
  try {
    await cleanup(document.pages.map((page) => page.path));
    document.pages = [];
    document.selected = null;
    render();
    recordHistory(document, "Document reset");
    setStatus("Scan cleared.", "success");
  } catch (error) {
    recordHistory(document, `Reset failed: ${String(error)}`);
    setStatus(String(error), "error");
  } finally {
    setBusy(false);
  }
}

function openUploadDialog(job = null) {
  const document = job ? documentById(job.documentId) : currentDocument();
  if (!document || document.pages.length === 0) {
    setStatus("Add at least one page first.", "error");
    return;
  }
  state.uploadDialogJob = job;
  state.uploadDialogDocumentId = document.id;
  $("upload-title").value = job ? job.title : "";
  showDialog($("upload-dialog"));
  requestAnimationFrame(() => $("upload-title").focus());
}

function upload() {
  jumpToScanDocument();
  if (state.settings.ask_for_filename) {
    openUploadDialog();
  } else {
    startUpload(currentDocument(), "");
  }
}

function nextDocumentId() {
  return `document-${state.nextDocumentNumber++}`;
}

function nextJobId() {
  return `upload-${state.nextJobNumber++}`;
}

function compactArchivePage(page) {
  const fallback = {
    preview: page.thumbnail || "",
    thumbnail: page.thumbnail,
    rotation: page.rotation
  };
  const source = page.preview || page.thumbnail;
  if (!source) return Promise.resolve(fallback);

  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => {
      try {
        const maxDimension = 1200;
        const scale = Math.min(1, maxDimension / Math.max(image.naturalWidth, image.naturalHeight));
        const canvas = document.createElement("canvas");
        canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
        canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
        canvas.getContext("2d").drawImage(image, 0, 0, canvas.width, canvas.height);
        resolve({ ...fallback, preview: canvas.toDataURL("image/jpeg", 0.1) });
      } catch {
        resolve(fallback);
      }
    };
    image.onerror = () => resolve(fallback);
    image.src = source;
  });
}

async function compactArchivePages(pages) {
  return Promise.all(pages.map((page) => compactArchivePage(page)));
}

function findJob(jobId) {
  return state.documents
    .map((document) => [document.upload, document.backgroundJob])
    .flat()
    .find((job) => job?.id === jobId) || null;
}

function startUpload(document, title, existingJob = null) {
  if (!document || document.pages.length === 0) {
    setStatus("Add at least one page first.", "error");
    return;
  }
  const job = existingJob || {
    id: nextJobId(),
    documentId: document.id,
    pages: [...document.pages],
    title,
    settings: JSON.parse(JSON.stringify(state.settings)),
    state: "active",
    progress: "Estimating PDF size…",
    error: "",
    viewDocumentId: null,
    history: [...(document.history || [])]
  };
  job.pages = [...document.pages];
  job.title = title;
  job.state = "active";
  job.progress = "Estimating PDF size…";
  job.error = "";
  job.documentId = document.id;
  recordHistory(job, "Estimating PDF size…", "update");
  // A retry must use settings saved since the previous attempt.
  job.settings = JSON.parse(JSON.stringify(state.settings));
  document.upload = job;

  if (!existingJob) {
    const next = createDocumentState(nextDocumentId());
    next.backgroundJob = job;
    job.viewDocumentId = next.id;
    state.documents.push(next);
    state.activeDocumentId = next.id;
    state.scanDocumentId = next.id;
  } else {
    const view = state.documents.find((candidate) => candidate.backgroundJob?.id === job.id);
    if (view) {
      view.backgroundJob = job;
      state.activeDocumentId = view.id;
    }
  }

  render();
  setStatus("Upload started.", "success");
  void invoke("upload_document", {
    paths: job.pages.map((page) => page.path),
    rotations: job.pages.map(pageRotation),
    settings: job.settings,
    title: job.title,
    jobId: job.id
  }).then(async (identifier) => {
    try {
      await cleanup(job.pages.map((page) => page.path));
    } catch (error) {
      console.warn("Uploaded, but could not clean up temporary pages", error);
    }
    recordHistory(job, "Uploaded successfully", "update");
    job.state = "success";
    job.fileIdentifier = identifier;
    job.progress = "Uploaded successfully";
    job.history = [];
    const archivePages = await compactArchivePages(job.pages);
    const archive = createArchivedDocumentState(`archive-${job.id}`, archivePages, job.fileIdentifier);
    job.pages = [];
    state.documents = limitArchivedDocumentsState([
      archive,
      ...removeDocumentState(state.documents, job.documentId)
        .filter((candidate) => candidate.id !== archive.id)
    ], 25);
    const view = documentById(job.viewDocumentId);
    if (view) view.backgroundJob = job;
    if (!documentById(state.activeDocumentId)) state.activeDocumentId = view?.id || state.documents.at(-1)?.id;
    render({ scrollToEnd: true });
    setStatus("Uploaded to Paperless.", "success");
    window.setTimeout(() => {
      if (view?.backgroundJob === job) {
        view.backgroundJob = null;
        render();
      }
    }, 1800);
  }).catch((error) => {
    job.state = "failed";
    job.progress = "Upload failed";
    job.error = String(error);
    recordHistory(job, `Upload failed: ${job.error}`, "update");
    render();
    setStatus(`Upload failed: ${job.error}`, "error");
  });
}

function rotatePreview() {
  jumpToScanDocument();
  const group = currentDocument();
  if (!group || group.selected === null || !group.pages[group.selected]) return;
  const page = group.pages[group.selected];
  rotatePageState(group, group.selected);
  renderPreview();
  const thumbnail = [...globalThis.document.querySelectorAll(
    `.thumbnail-frame[data-document-id="${CSS.escape(group.id)}"][data-page-index="${group.selected}"] img`
  )][0];
  if (thumbnail) applyPageRotation(thumbnail, page);
  recordHistory(group, "Page rotated 90 degrees");
  setStatus("Page rotated 90°.", "success");
}

async function removePage(documentId, index, expectedPath = "") {
  const group = documentById(documentId);
  if (state.busy || !group || group.upload) return;
  const actualIndex = expectedPath
    ? group.pages.findIndex((page) => page.path === expectedPath)
    : index;
  if (actualIndex < 0 || !group.pages[actualIndex]) return;
  const [page] = group.pages.splice(actualIndex, 1);
  try {
    await cleanup([page.path]);
    if (group.pages.length === 0) group.selected = null;
    else if (group.selected === actualIndex) group.selected = Math.min(actualIndex, group.pages.length - 1);
    else if (group.selected > actualIndex) group.selected -= 1;
    recordHistory(group, "Page removed");
    render();
    setStatus("Page removed.", "success");
  } catch (error) {
    group.pages.splice(actualIndex, 0, page);
    render();
    setStatus(String(error), "error");
  }
}

function fillSettingsForm() {
  $("scanner-resolution").value = String(state.settings.scanner.resolution);
  $("scanner-mode").value = state.settings.scanner.mode;
  $("paperless-url").value = state.settings.paperless_url;
  $("paperless-token").value = state.settings.paperless_token;
  $("compression").value = String(state.settings.compression);
  $("compression-value").textContent = `${state.settings.compression}%`;
  $("compression-format").value = state.settings.compression_format || "jpeg";
  $("paper-format").value = state.settings.paper_format || "a4";
  $("theme").value = normalizedTheme(state.settings.theme);
  $("max-upload-size").value = String(state.settings.max_upload_size_mb || 10);
  $("simple-mode").checked = Boolean(state.settings.simple_mode);
  $("ask-for-filename").checked = state.settings.ask_for_filename !== false;
  $("hash-file-naming").checked = state.settings.hash_file_naming !== false;
  $("debug-history").checked = Boolean(state.settings.debug_history);
  updatePaperlessUrlWarning();
}

function updatePaperlessUrlWarning() {
  const warning = $("paperless-url-warning");
  if (!warning) return;
  let insecure = false;
  try {
    insecure = new URL($("paperless-url").value.trim()).protocol === "http:";
  } catch {
    insecure = false;
  }
  warning.hidden = !insecure;
}

async function refreshScanners() {
  const select = $("scanner-device");
  const refreshButton = $("refresh-scanners");
  const scannerStatus = $("scanner-status");
  select.replaceChildren(new Option("Default scanner", ""));
  refreshButton.disabled = true;
  scannerStatus.textContent = "Looking for scanners…";
  try {
    const scanners = await invoke("list_scanners");
    scanners.forEach((scanner) => select.append(new Option(scanner, scanner)));
    select.value = state.settings.scanner.device;
    scannerStatus.textContent = scanners.length === 0
      ? "No scanners found. Check SANE and try again."
      : `${scanners.length} scanner${scanners.length === 1 ? "" : "s"} found.`;
  } catch (error) {
    scannerStatus.textContent = `Scanner lookup failed: ${String(error)}`;
    setStatus(String(error), "error");
  } finally {
    refreshButton.disabled = false;
  }
}

async function openSettings() {
  fillSettingsForm();
  showDialog($("settings-dialog"));
  await refreshScanners();
}

async function saveSettings(event) {
  event.preventDefault();
  state.settings = {
    scanner: {
      device: $("scanner-device").value,
      resolution: Number($("scanner-resolution").value),
      mode: $("scanner-mode").value
    },
    paperless_url: $("paperless-url").value.trim(),
    paperless_token: $("paperless-token").value.trim(),
    compression: Number($("compression").value),
    compression_format: $("compression-format").value,
    paper_format: $("paper-format").value,
    theme: normalizedTheme($("theme").value),
    max_upload_size_mb: Math.max(1, Number($("max-upload-size").value) || 10),
    simple_mode: $("simple-mode").checked,
    ask_for_filename: $("ask-for-filename").checked,
    hash_file_naming: $("hash-file-naming").checked,
    debug_history: $("debug-history").checked
  };
  try {
    await invoke("save_settings", { settings: state.settings });
    applyTheme();
    if (state.settings.simple_mode) {
      const document = currentDocument();
      if (document) {
        selectLastPageState(document);
        state.activeDocumentId = document.id;
      }
    }
    closeDialog($("settings-dialog"));
    render();
    setStatus("Settings saved.", "success");
  } catch (error) {
    setStatus(String(error), "error");
  }
}

function listenForUploadProgress() {
  const eventApi = window.__TAURI__.event;
  if (!eventApi?.listen) return;
  void eventApi.listen("upload-progress", (event) => {
    const payload = event.payload || {};
    const job = findJob(payload.job_id);
    if (!job || job.state !== "active") return;
    job.progress = payload.stage;
    recordHistory(job, payload.stage, "update");
    renderUploadStatus();
  });
}

// WebKitGTK can handle pinch-to-zoom as a native webview gesture before the
// page's pointer handlers see it. Cancel that default globally; the preview's
// pointer handlers still provide its own two-finger zoom.
function preventBrowserPinchZoom(event) {
  const multiTouch = (event.type === "touchstart" || event.type === "touchmove")
    && event.touches?.length > 1;
  const nativeGesture = event.type.startsWith("gesture");
  if (multiTouch || nativeGesture) event.preventDefault();
}

document.addEventListener("touchstart", preventBrowserPinchZoom, { capture: true, passive: false });
document.addEventListener("touchmove", preventBrowserPinchZoom, { capture: true, passive: false });
document.addEventListener("gesturestart", preventBrowserPinchZoom, { capture: true, passive: false });
document.addEventListener("gesturechange", preventBrowserPinchZoom, { capture: true, passive: false });
document.addEventListener("gestureend", preventBrowserPinchZoom, { capture: true, passive: false });

$("add-page").addEventListener("click", addPage);
$("rescan").addEventListener("click", rescan);
$("upload").addEventListener("click", () => {
  try {
    upload();
  } catch (error) {
    console.error("Could not open upload dialog", error);
    setStatus(`Could not open upload dialog: ${String(error)}`, "error");
  }
});
$("reset").addEventListener("click", reset);
$("settings-button").addEventListener("click", openSettings);
$("rotate-preview").addEventListener("click", rotatePreview);
$("document-groups").addEventListener("scroll", scheduleArchiveBoundarySnap, { passive: true });
$("zoom-in").addEventListener("click", () => changePreviewZoom(1));
$("zoom-out").addEventListener("click", () => changePreviewZoom(-1));
$("preview").addEventListener("pointerdown", previewPointerDown);
$("preview").addEventListener("pointermove", previewPointerMove);
$("preview").addEventListener("pointerup", previewPointerUp);
$("preview").addEventListener("pointercancel", previewPointerUp);
$("preview").addEventListener("wheel", previewWheel, { passive: false });
$("refresh-scanners").addEventListener("click", refreshScanners);
document.querySelectorAll(".close-button").forEach((button) => {
  button.addEventListener("click", () => closeDialog(button.closest(".dialog")));
});
document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  const dialogs = [...document.querySelectorAll(".dialog:not([hidden])")];
  const dialog = dialogs.at(-1);
  if (!dialog) return;
  if (dialog.id === "confirm-dialog") closeConfirmDialog();
  else closeDialog(dialog);
});
$("cancel-upload").addEventListener("click", () => closeDialog($("upload-dialog")));
$("cancel-confirm").addEventListener("click", closeConfirmDialog);
$("confirm-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const onConfirm = state.confirmation;
  closeConfirmDialog();
  onConfirm?.();
});
$("settings-form").addEventListener("submit", saveSettings);
$("paperless-url").addEventListener("input", updatePaperlessUrlWarning);
$("compression").addEventListener("input", (event) => {
  $("compression-value").textContent = `${event.target.value}%`;
});
$("upload-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const title = $("upload-title").value;
  const retryJob = state.uploadDialogJob;
  const document = documentById(state.uploadDialogDocumentId);
  state.uploadDialogJob = null;
  state.uploadDialogDocumentId = null;
  closeDialog($("upload-dialog"));
  startUpload(document, title, retryJob);
});

listenForUploadProgress();
invoke("load_settings")
  .then((settings) => {
    state.settings = {
      ...state.settings,
      ...settings,
      scanner: { ...state.settings.scanner, ...settings.scanner },
      theme: normalizedTheme(settings.theme)
    };
    applyTheme();
    render();
  })
  .catch((error) => setStatus(String(error), "error"));
invoke("restore_pages")
  .then((pages) => {
    if (pages.length) {
      state.documents = [createDocumentState("document-1", pages)];
      state.activeDocumentId = "document-1";
      state.nextDocumentNumber = 2;
    }
    render();
    if (pages.length) setStatus(`Resumed ${pages.length} saved page${pages.length === 1 ? "" : "s"}.`, "success");
  })
  .catch((error) => setStatus(String(error), "error"));
render();
