// Stands in for the webview, so a screen's file can be imported to test its logic.
const commands = () => Promise.resolve(null);

globalThis.window = {
  __TAURI__: {
    core: { invoke: commands, Channel: class {} },
    event: { listen: commands },
    path: { sep: () => "/" },
  },
};
globalThis.document = { querySelector: () => null, addEventListener() {} };
