function detachPagesForUpload(pages) {
  return {
    uploadPages: [...pages],
    currentPages: [],
    selected: null
  };
}

function showDialog(dialog) {
  dialog.hidden = false;
  dialog.setAttribute("aria-hidden", "false");
}

function closeDialog(dialog) {
  dialog.hidden = true;
  dialog.setAttribute("aria-hidden", "true");
}

if (typeof module !== "undefined") {
  module.exports = { detachPagesForUpload, showDialog, closeDialog };
}
