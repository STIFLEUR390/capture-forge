// js/chrome_shim.js
// Shim for Chrome APIs not yet exposed via web-sys.
// Used by the Rust stream acquisition module via wasm-bindgen.

// Tab capture — returns a Promise<{ streamId: string }>.
// Uses chrome.tabCapture.getMediaStreamId() to produce a chromeMediaSourceId
// that can be passed to getUserMedia() in the offscreen document for stream
// reconstruction.  chrome.tabCapture is only available from the service worker
// background context, not from offscreen documents.
export function tabCaptureCapture() {
    return new Promise((resolve, reject) => {
        chrome.tabCapture.getMediaStreamId({}, (response) => {
            if (chrome.runtime.lastError) {
                reject(new Error(chrome.runtime.lastError.message));
            } else {
                resolve({ streamId: response.streamId });
            }
        });
    });
}
