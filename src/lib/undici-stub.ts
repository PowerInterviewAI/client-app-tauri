// Stub for the `undici` package, aliased in vite.config.ts.
//
// `@mohtasham/md-to-docx` statically imports `{ Agent, buildConnector }` from `undici`
// for its SSRF-protected remote-image-fetch path (secureImageFetch.js), which this app
// never enables (convertMarkdownToDocx is called without image.remote options, so that
// path always throws "Remote image fetching is disabled" before these are used).
// Bundling the real `undici` pulls in Node-only internals (node:util, node:dns,
// node:net, etc.) that crash in the Tauri webview at module-evaluation time, e.g.
// "util.debuglog is not a function". This stub satisfies the static import without
// pulling any of that in.

export class Agent {
  close(): Promise<void> {
    return Promise.resolve();
  }
}

export function buildConnector(): (...args: unknown[]) => void {
  return () => {};
}
