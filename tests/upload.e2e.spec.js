const { test, expect } = require("@playwright/test");

async function openApp(page, options = {}) {
  const pages = options.pages || [{
    path: "/tmp/page-1.png",
    preview: "data:image/png;base64,test"
  }];
  await page.addInitScript(({ pages: restoredPages, simpleMode, scanError, scanPending }) => {
    const calls = [];
    window.__testCalls = calls;
    window.__TAURI__ = {
      core: {
        invoke(command, args) {
          calls.push({ command, args });
          if (command === "load_settings") {
            return Promise.resolve({
              scanner: { device: "", resolution: 300, mode: "Color" },
              paperless_url: "http://paperless.test",
              paperless_token: "test-token",
              compression: 85,
              compression_format: "jpeg",
              paper_format: "a4",
              max_upload_size_mb: 10,
              simple_mode: simpleMode,
              ask_for_filename: true,
              hash_file_naming: true
            });
          }
          if (command === "restore_pages") {
            return Promise.resolve(restoredPages);
          }
          if (command === "scan_page") {
            if (scanError) return Promise.reject(scanError);
            if (scanPending) return new Promise((resolve) => { window.__scanResolve = resolve; });
            return Promise.resolve({
              path: "/tmp/scanned-page.png",
              preview: "data:image/png;base64,scanned-page"
            });
          }
          if (command === "rotate_page") {
            return Promise.resolve({
              path: args.path,
              preview: "data:image/png;base64,rotated-page"
            });
          }
          if (command === "cleanup_pages" || command === "save_settings") {
            return Promise.resolve();
          }
          if (command === "upload_document") {
            return new Promise((resolve, reject) => {
              window.__uploadResolvers[args.jobId] = resolve;
              window.__uploadRejectors[args.jobId] = reject;
            });
          }
          if (command === "list_scanners") {
            return Promise.resolve([]);
          }
          throw new Error(`Unexpected Tauri command: ${command}`);
        }
      },
      event: {
        listen: async (name, callback) => {
          if (name === "upload-progress") window.__uploadProgress = callback;
          return () => {};
        }
      }
    };
    window.__uploadResolvers = {};
    window.__uploadRejectors = {};
  }, {
    pages,
    simpleMode: Boolean(options.simpleMode),
    scanError: options.scanError || "",
    scanPending: Boolean(options.scanPending)
  });
  await page.goto("/index.html");
  await expect(page.locator("#preview img")).toBeVisible();
}

test("Enter opens upload and submits it in the background", async ({ page }) => {
  await openApp(page);
  await expect(page.locator("#upload-status")).toBeVisible();
  const actionsBeforeUpload = await page.locator("#actions").boundingBox();

  await page.locator("#upload").focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#upload-dialog")).toBeVisible();

  await page.locator("#upload-title").fill("test document");
  await page.locator("#upload-title").press("Enter");

  await expect.poll(async () => page.evaluate(() =>
    window.__testCalls.filter(({ command }) => command === "upload_document").length
  )).toBe(1);
  await expect(page.locator("#upload-dialog")).toBeHidden();
  await expect(page.locator("#document-groups .document-group")).toHaveCount(2);
  await expect(page.locator("#document-groups .document-group.uploading")).toHaveCount(1);
  await expect(page.locator("#document-groups .document-group.active .empty-message")).toHaveText("No pages yet. Add a page to start.");
  await expect(page.locator("#preview")).toContainText("Your current page will appear here");
  await expect(page.locator("#upload-status")).toBeVisible();
  await expect(page.locator("#upload-status-text")).toHaveText("Estimating PDF size…");
  const actionsDuringUpload = await page.locator("#actions").boundingBox();
  expect(actionsDuringUpload.height).toBe(actionsBeforeUpload.height);
  await page.locator("#upload-status-text").click();
  await expect(page.locator("#status-dialog")).toBeVisible();
  await expect(page.locator("#status-history-list")).toContainText("Estimating PDF size");
  await page.keyboard.press("Escape");
  await expect(page.locator("#status-dialog")).toBeHidden();

  const upload = await page.evaluate(() =>
    window.__testCalls.find(({ command }) => command === "upload_document")
  );
  expect(upload.args.paths).toEqual(["/tmp/page-1.png"]);
  expect(upload.args.rotations).toEqual([0]);
  expect(upload.args.title).toBe("test document");

  await page.evaluate((jobId) => window.__uploadProgress({
    payload: { job_id: jobId, stage: "Uploading to Paperless…" }
  }), upload.args.jobId);
  await expect(page.locator("#upload-status-text")).toHaveText("Uploading to Paperless…");

  await page.evaluate((jobId) => window.__uploadResolvers[jobId]("aB12cD34"), upload.args.jobId);
  await expect(page.locator("#upload-status-text")).toHaveText("Uploaded successfully");
  await expect(page.locator("#status")).toHaveText("Uploaded to Paperless.");
  await expect(page.locator("#document-groups .document-group")).toHaveCount(2);
  await expect(page.locator("#document-groups .document-group.archived")).toHaveCount(1);
  await expect(page.locator("#document-groups .document-group.archived .thumbnail")).toHaveCount(1);
  await expect(page.locator("#document-groups .document-group.archived .document-group-header")).toHaveCount(1);
  await expect(page.locator("#document-groups .document-group.archived .archive-identifier")).toHaveText("aB12cD34");
  await expect(page.locator("#document-groups .document-group.archived")).toHaveCSS("border-color", "rgb(47, 158, 91)");
  await page.locator("#document-groups .document-group.archived .archive-identifier").click();
  await expect(page.locator("#document-groups .document-group.archived")).toHaveClass(/active/);
  await page.locator("#document-groups").evaluate((tabs) => { tabs.scrollLeft = tabs.scrollWidth; });
  expect(await page.evaluate(() => {
    const archive = document.querySelector("#document-groups .document-group.archived").getBoundingClientRect();
    const current = document.querySelector("#document-groups .document-group:not(.archived)").getBoundingClientRect();
    return current.left - archive.right;
  })).toBeLessThan(20);
  expect(await page.evaluate(() => {
    const archive = document.querySelector("#document-groups .document-group.archived").getBoundingClientRect();
    const current = document.querySelector("#document-groups .document-group:not(.archived)").getBoundingClientRect();
    return archive.width < current.width;
  })).toBe(true);
  await expect.poll(async () => page.evaluate(() => {
    const strip = document.querySelector("#document-groups").getBoundingClientRect();
    const archive = document.querySelector("#document-groups .document-group.archived").getBoundingClientRect();
    return archive.right <= strip.left || archive.left >= strip.right;
  })).toBe(true);
  expect(await page.evaluate(() => {
    const strip = document.querySelector("#document-groups").getBoundingClientRect();
    const archive = document.querySelector("#document-groups .document-group.archived").getBoundingClientRect();
    return archive.right <= strip.left || archive.left >= strip.right;
  })).toBe(true);

  const boundary = await page.evaluate(() => {
    const tabs = document.querySelector("#document-groups");
    const current = tabs.querySelector(".document-group:not(.archived)");
    const tabsBox = tabs.getBoundingClientRect();
    const currentBox = current.getBoundingClientRect();
    return Math.min(
      tabs.scrollWidth - tabs.clientWidth,
      currentBox.left - tabsBox.left + tabs.scrollLeft
    );
  });
  await page.evaluate((nearBoundary) => {
    const tabs = document.querySelector("#document-groups");
    tabs.scrollLeft = nearBoundary - 40;
    tabs.dispatchEvent(new Event("scroll"));
  }, boundary);
  await expect.poll(async () => page.evaluate(() => document.querySelector("#document-groups").scrollLeft), {
    timeout: 2_000
  }).toBeGreaterThanOrEqual(boundary - 2);

  await page.reload();
  await expect(page.locator("#document-groups .document-group.archived")).toHaveCount(0);
});

test("archived large previews use a compact in-memory JPEG and actions return to the scan document", async ({ page }) => {
  const image = `data:image/svg+xml,${encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900"><rect width="100%" height="100%" fill="tomato"/></svg>'
  )}`;
  await openApp(page, {
    pages: [{ path: "/tmp/page-1.png", preview: image, thumbnail: image }]
  });

  await page.locator("#upload").click();
  await page.locator("#upload-title").press("Enter");
  const upload = await page.evaluate(() => window.__testCalls.find(({ command }) => command === "upload_document"));
  await page.evaluate((jobId) => window.__uploadResolvers[jobId]("aB12cD34"), upload.args.jobId);
  await expect(page.locator("#document-groups .document-group.archived")).toHaveCount(1);

  await page.locator(".archive-identifier").click();
  await expect(page.locator("#preview img")).toHaveAttribute("src", /^data:image\/jpeg;base64,/);
  await page.locator("#add-page").click();
  await expect(page.locator("#preview img")).toHaveAttribute("src", /scanned-page/);
  await expect(page.locator("#document-groups .document-group:not(.archived).active")).toHaveCount(1);
});

test("retrying a failed upload uses the newly saved compression setting", async ({ page }) => {
  await openApp(page);

  await page.locator("#upload").focus();
  await page.keyboard.press("Enter");
  await page.locator("#upload-title").press("Enter");
  const firstUpload = await page.evaluate(() => window.__testCalls.find(({ command }) => command === "upload_document"));
  await page.evaluate((jobId) => window.__uploadRejectors[jobId]("Paperless rejected the upload (413 Payload Too Large)"), firstUpload.args.jobId);
  await expect(page.locator("#upload-status-text")).toContainText("Upload failed");

  await page.locator("#settings-button").click();
  await page.locator("#compression").fill("40");
  await page.locator("#save-settings").click();
  await page.locator("#upload-indicator").click();
  await page.locator("#upload-title").press("Enter");

  const uploads = await page.evaluate(() => window.__testCalls
    .filter(({ command }) => command === "upload_document")
    .map(({ args }) => args));
  expect(uploads).toHaveLength(2);
  expect(uploads[1].settings.compression).toBe(40);
});

test("simple mode permits multiple background uploads while keeping one active group", async ({ page }) => {
  await openApp(page, { simpleMode: true });

  async function submitUpload(title) {
    await page.locator("#upload").focus();
    await page.keyboard.press("Enter");
    await expect(page.locator("#upload-dialog")).toBeVisible();
    await page.locator("#upload-title").fill(title);
    await page.locator("#upload-title").press("Enter");
  }

  await submitUpload("first");
  await page.locator("#add-page").click();
  await expect(page.locator("#preview img")).toHaveAttribute("src", /scanned-page/);
  await submitUpload("second");

  await expect.poll(async () => page.evaluate(() =>
    window.__testCalls.filter(({ command }) => command === "upload_document").length
  )).toBe(2);
  await expect(page.locator("#document-groups .document-group:not(.archived)")).toHaveCount(1);
  await expect(page.locator("#document-groups .document-group:not(.archived) .document-group-header")).toHaveCount(0);

  const uploads = await page.evaluate(() => window.__testCalls
    .filter(({ command }) => command === "upload_document")
    .map(({ args }) => ({ id: args.jobId, title: args.title })));
  expect(uploads.map(({ title }) => title)).toEqual(["first", "second"]);

  for (const { id } of uploads) {
    await page.evaluate((jobId) => window.__uploadResolvers[jobId]("OK"), id);
  }
});

test("tab bar follows the newest live document when many uploads are active", async ({ page }) => {
  await openApp(page);

  async function submitUpload(title) {
    await page.locator("#upload").click();
    await expect(page.locator("#upload-dialog")).toBeVisible();
    await page.locator("#upload-title").fill(title);
    await page.locator("#upload-title").press("Enter");
  }

  await submitUpload("first");
  await page.locator("#add-page").click();
  await submitUpload("second");

  await expect.poll(async () => page.evaluate(() =>
    window.__testCalls.filter(({ command }) => command === "upload_document").length
  )).toBe(2);
  await expect.poll(async () => page.evaluate(() => {
    const tabs = document.querySelector("#document-groups");
    return tabs.scrollLeft === tabs.scrollWidth - tabs.clientWidth;
  })).toBe(true);
  expect(await page.evaluate(() => {
    const tabs = document.querySelector("#document-groups").getBoundingClientRect();
    const newest = document.querySelectorAll("#document-groups .document-group:not(.archived)");
    const box = newest[newest.length - 1].getBoundingClientRect();
    return box.left >= tabs.left && box.right <= tabs.right;
  })).toBe(true);
});

test("switching page tabs allows inserting a scan in the middle", async ({ page }) => {
  await openApp(page, {
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" },
      { path: "/tmp/page-3.png", preview: "data:image/png;base64,three" }
    ]
  });

  await page.locator(".document-group .thumbnail").nth(0).click();
  await page.locator("#add-page").click();

  await expect(page.locator(".document-group .thumbnail")).toHaveCount(4);
  const sources = await page.locator(".document-group .thumbnail img").evaluateAll((images) => images.map((image) => image.src));
  expect(sources[1]).toContain("scanned-page");
  await expect(page.locator("#preview img")).toHaveAttribute("src", /scanned-page/);
});

test("dragging page tabs changes their order", async ({ page }) => {
  await openApp(page, {
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" },
      { path: "/tmp/page-3.png", preview: "data:image/png;base64,three" }
    ]
  });

  const tabs = page.locator(".document-group .thumbnail-frame");
  await page.evaluate(() => {
    window.__thumbnailFrames = [...document.querySelectorAll(".document-group .thumbnail-frame")];
  });
  const firstBox = await tabs.nth(0).boundingBox();
  const lastBox = await tabs.nth(2).boundingBox();
  await page.mouse.move(firstBox.x + firstBox.width / 2, firstBox.y + firstBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(lastBox.x + lastBox.width / 2, lastBox.y + lastBox.height / 2, { steps: 8 });
  await page.mouse.up();

  const sources = await page.locator(".document-group .thumbnail img").evaluateAll((images) => images.map((image) => image.src));
  expect(sources).toEqual([
    expect.stringContaining("two"),
    expect.stringContaining("three"),
    expect.stringContaining("one")
  ]);
  expect(await page.evaluate(() => window.__thumbnailFrames.every((frame) => frame.isConnected))).toBe(true);
});

test("clicking an upload group switches the preview to that group", async ({ page }) => {
  await openApp(page);
  await page.locator("#upload").focus();
  await page.keyboard.press("Enter");
  await page.locator("#upload-title").press("Enter");

  await page.locator(".document-group.uploading").click();
  await expect(page.locator(".document-group.uploading.active")).toHaveCount(1);
  await expect(page.locator("#preview img")).toHaveAttribute("src", /test/);
});

test("the action buttons sit beside the large preview", async ({ page }) => {
  await openApp(page);
  const preview = await page.locator(".preview-panel").boundingBox();
  const actions = await page.locator("#actions").boundingBox();
  expect(actions.x).toBeGreaterThanOrEqual(preview.x + preview.width - 1);
});

test("rotate updates the preview immediately and is included in the upload", async ({ page }) => {
  await openApp(page);
  await page.locator("#rotate-preview").click();

  await expect.poll(async () => page.locator("#preview img").evaluate((image) => image.style.transform))
    .toContain("rotate(90deg)");
  expect(await page.evaluate(() => window.__testCalls.filter(({ command }) => command === "rotate_page").length)).toBe(0);

  await page.locator("#upload").click();
  await page.locator("#upload-title").press("Enter");
  const upload = await page.evaluate(() => window.__testCalls.find(({ command }) => command === "upload_document"));
  expect(upload.args.rotations).toEqual([90]);
});

test("page rotation survives switching away and back", async ({ page }) => {
  await openApp(page, {
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" }
    ]
  });

  await page.locator("#rotate-preview").click();
  await expect.poll(async () => page.locator("#preview img").evaluate((image) => image.style.transform))
    .toContain("rotate(90deg)");

  await page.locator(".document-group .thumbnail").nth(0).click();
  await page.locator(".document-group .thumbnail").nth(1).click();

  await expect.poll(async () => page.locator("#preview img").evaluate((image) => image.style.transform))
    .toContain("rotate(90deg)");
  await expect(page.locator(".document-group .thumbnail").nth(1).locator("img"))
    .toHaveCSS("transform", /matrix/);
});

test("large preview supports button and wheel zoom with drag panning", async ({ page }) => {
  const image = `data:image/svg+xml,${encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900"><rect width="100%" height="100%" fill="steelblue"/></svg>'
  )}`;
  await openApp(page, { pages: [{ path: "/tmp/large-page.png", preview: image, thumbnail: image }] });
  const rotateBox = await page.locator("#rotate-preview").boundingBox();
  const zoomBox = await page.locator("#zoom-controls").boundingBox();
  const zoomOutBox = await page.locator("#zoom-out").boundingBox();
  const zoomInBox = await page.locator("#zoom-in").boundingBox();
  expect(zoomBox.y).toBeGreaterThanOrEqual(rotateBox.y + rotateBox.height + 8);
  expect(Math.abs((zoomBox.x + zoomBox.width) - (rotateBox.x + rotateBox.width))).toBeLessThanOrEqual(1);
  expect(zoomInBox.x).toBeCloseTo(zoomOutBox.x, 0);
  expect(zoomInBox.y).toBeGreaterThan(zoomOutBox.y);
  await expect(page.locator("#zoom-controls")).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await expect(page.locator("#zoom-controls")).toHaveCSS("box-shadow", "none");
  expect(await page.evaluate(() => {
    const label = Number.parseFloat(getComputedStyle(document.querySelector("#zoom-level")).fontSize);
    const button = Number.parseFloat(getComputedStyle(document.querySelector("#zoom-in")).fontSize);
    return label < button;
  })).toBe(true);
  await expect(page.locator("meta[name='viewport']")).toHaveAttribute("content", /user-scalable=no/);
  await expect(page.locator("body")).toHaveCSS("touch-action", "pan-x pan-y");
  await expect(page.locator("#preview")).toHaveCSS("touch-action", "none");
  expect(await page.evaluate(() => {
    const gesture = new Event("gesturestart", { bubbles: true, cancelable: true });
    document.dispatchEvent(gesture);
    const touchMove = new Event("touchmove", { bubbles: true, cancelable: true });
    Object.defineProperty(touchMove, "touches", { value: [{}, {}] });
    document.body.dispatchEvent(touchMove);
    return gesture.defaultPrevented && touchMove.defaultPrevented;
  })).toBe(true);

  const preview = page.locator("#preview");
  await expect(page.locator("#zoom-level")).toHaveText("100%");
  await page.locator("#zoom-in").click();
  await expect(page.locator("#zoom-level")).toHaveText("125%");
  await expect.poll(async () => preview.locator("img").evaluate((image) => image.style.transform))
    .toContain("scale(1.25)");

  await preview.hover();
  await page.mouse.wheel(0, -100);
  await expect(page.locator("#zoom-level")).toHaveText("150%");

  const box = await preview.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 40, box.y + box.height / 2 + 25);
  await page.mouse.up();
  await expect.poll(async () => preview.locator("img").evaluate((image) => image.style.transform))
    .toContain("translate(40px, 25px)");

  await page.locator("#zoom-out").click();
  await expect(page.locator("#zoom-level")).toHaveText("125%");
});

test("large preview supports pinch-style pointer zoom", async ({ page }) => {
  const image = `data:image/svg+xml,${encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900"><rect width="100%" height="100%" fill="steelblue"/></svg>'
  )}`;
  await openApp(page, { pages: [{ path: "/tmp/large-page.png", preview: image, thumbnail: image }] });

  await page.evaluate(() => {
    const preview = document.querySelector("#preview");
    const send = (type, pointerId, clientX, clientY) => preview.dispatchEvent(new PointerEvent(type, {
      bubbles: true,
      pointerId,
      pointerType: "touch",
      clientX,
      clientY
    }));
    send("pointerdown", 1, 500, 350);
    send("pointerdown", 2, 540, 350);
    send("pointermove", 2, 600, 350);
    send("pointerup", 1, 500, 350);
    send("pointerup", 2, 600, 350);
  });

  await expect(page.locator("#zoom-level")).toHaveText("250%");
});

test("selecting a page does not rebuild all thumbnail images", async ({ page }) => {
  await openApp(page, {
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" },
      { path: "/tmp/page-3.png", preview: "data:image/png;base64,three" }
    ]
  });

  await page.evaluate(() => {
    window.__thumbnailToKeep = document.querySelector(".document-group .thumbnail img");
  });
  await page.locator(".document-group .thumbnail").nth(1).click();

  expect(await page.evaluate(() => window.__thumbnailToKeep.isConnected)).toBe(true);
  await expect(page.locator("#preview img")).toHaveAttribute("src", /two/);
});

test("simple mode shows one non-switchable document", async ({ page }) => {
  await openApp(page, {
    simpleMode: true,
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" }
    ]
  });

  await expect(page.locator("#document-groups .document-group")).toHaveCount(1);
  await expect(page.locator(".document-group-header")).toHaveCount(0);
  await expect(page.locator(".document-group .thumbnail:disabled")).toHaveCount(2);
  await expect(page.locator("#preview img")).toHaveAttribute("src", /two/);
});

test("simple mode can be enabled from Settings", async ({ page }) => {
  await openApp(page);

  await page.locator("#settings-button").click();
  await expect(page.locator("#settings-dialog")).toBeVisible();
  await page.locator("#simple-mode").check();
  await page.locator("#paper-format").selectOption("us-letter");
  await page.locator("#max-upload-size").fill("12");
  await page.locator("#save-settings").click();

  const save = await page.evaluate(() =>
    window.__testCalls.find(({ command }) => command === "save_settings")
  );
  expect(save.args.settings.simple_mode).toBe(true);
  expect(save.args.settings.paper_format).toBe("us-letter");
  expect(save.args.settings.max_upload_size_mb).toBe(12);
  await expect(page.locator(".document-group-header")).toHaveCount(0);
});

test("removing a page tab requires confirmation and removes that page", async ({ page }) => {
  await openApp(page, {
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" }
    ]
  });

  await page.locator(".thumb-remove").nth(0).click();
  await expect(page.locator("#confirm-dialog")).toBeVisible();
  await expect(page.locator(".document-group .thumbnail-frame")).toHaveCount(2);
  await page.keyboard.press("Escape");
  await expect(page.locator(".document-group .thumbnail-frame")).toHaveCount(2);

  await page.locator(".thumb-remove").nth(0).click();
  await page.locator("#confirm-action").click();
  await expect(page.locator(".document-group .thumbnail-frame")).toHaveCount(1);
  await expect(page.locator(".document-group .thumbnail img")).toHaveAttribute("src", /two/);
  await expect.poll(async () => page.evaluate(() =>
    window.__testCalls.filter(({ command }) => command === "cleanup_pages").length
  )).toBe(1);
});

test("filename prompt can be disabled in Settings", async ({ page }) => {
  await openApp(page);

  await page.locator("#settings-button").click();
  await expect(page.locator("#hash-file-naming")).toBeChecked();
  await page.locator("#ask-for-filename").uncheck();
  await page.locator("#hash-file-naming").uncheck();
  await page.locator("#save-settings").click();

  await page.locator("#upload").click();
  await expect(page.locator("#upload-dialog")).toBeHidden();
  await expect.poll(async () => page.evaluate(() =>
    window.__testCalls.filter(({ command }) => command === "upload_document").length
  )).toBe(1);

  const save = await page.evaluate(() => window.__testCalls.find(({ command }) => command === "save_settings"));
  expect(save.args.settings.ask_for_filename).toBe(false);
  expect(save.args.settings.hash_file_naming).toBe(false);
  const upload = await page.evaluate(() => window.__testCalls.find(({ command }) => command === "upload_document"));
  expect(upload.args.title).toBe("");
});

test("scanner activity does not disable unrelated controls", async ({ page }) => {
  await openApp(page, { scanPending: true });

  await page.locator("#add-page").click();
  await expect(page.locator("#add-page")).toBeDisabled();
  await expect(page.locator("#rescan")).toBeDisabled();
  await expect(page.locator("#upload")).toBeEnabled();
  await expect(page.locator("#reset")).toBeEnabled();
  await expect(page.locator("#settings-button")).toBeEnabled();

  await page.locator("#settings-button").click();
  await expect(page.locator("#settings-dialog")).toBeVisible();
  await page.locator(".close-button").first().click();
  await page.evaluate(() => window.__scanResolve({
    path: "/tmp/scanned-page.png",
    preview: "data:image/png;base64,scanned-page"
  }));
  await expect(page.locator("#preview img")).toHaveAttribute("src", /scanned-page/);
});

test("rescan and reset require touch-friendly confirmation", async ({ page }) => {
  await openApp(page, {
    pages: [
      { path: "/tmp/page-1.png", preview: "data:image/png;base64,one" },
      { path: "/tmp/page-2.png", preview: "data:image/png;base64,two" }
    ]
  });

  await page.locator("#rescan").click();
  await expect(page.locator("#confirm-dialog")).toBeVisible();
  await expect(page.locator("#confirm-action")).toHaveCSS("min-height", "64px");
  await page.keyboard.press("Escape");
  await expect(page.locator("#confirm-dialog")).toBeHidden();
  expect(await page.evaluate(() => window.__testCalls.filter(({ command }) => command === "scan_page").length)).toBe(0);

  await page.locator("#reset").click();
  await expect(page.locator("#confirm-dialog")).toBeVisible();
  await page.locator("#confirm-action").click();
  await expect(page.locator("#confirm-dialog")).toBeHidden();
  await expect(page.locator("#preview")).toContainText("Your current page will appear here");
  await expect.poll(async () => page.evaluate(() =>
    window.__testCalls.filter(({ command }) => command === "cleanup_pages").length
  )).toBe(1);
});

test("scanner errors are shown as scanner status", async ({ page }) => {
  await openApp(page, { scanError: "Scanner unavailable or disconnected" });

  await page.locator("#add-page").click();
  await expect(page.locator("#status")).toContainText("Scanner unavailable");
  await expect(page.locator("#scanner-status")).toContainText("Scanner unavailable");
  await expect(page.locator("#add-page")).toBeEnabled();
});

test("debug history shows scan states and Escape closes Settings", async ({ page }) => {
  await openApp(page);

  await page.locator("#settings-button").click();
  await page.locator("#debug-history").check();
  await page.locator("#save-settings").click();
  await page.locator("#add-page").click();
  await page.locator("#upload-status").click();
  await expect(page.locator("#status-dialog")).toContainText("Scan started");
  await page.keyboard.press("Escape");

  await page.locator("#settings-button").click();
  await page.keyboard.press("Escape");
  await expect(page.locator("#settings-dialog")).toBeHidden();
});
