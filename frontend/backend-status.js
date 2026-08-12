function backendWarningText(status) {
  if (!status?.experimental) return "";
  return status.warning || `Untested scanner backend (alpha, highly experimental): ${status.name || "unknown backend"}.`;
}

if (typeof module !== "undefined") module.exports = { backendWarningText };
else globalThis.backendWarningText = backendWarningText;
