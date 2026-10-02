// Shows one screen at a time. Each screen module exports `render(root, nav, ...args)`
// and builds its own DOM inside `root`.
import * as home from "./screens/home.js";
import * as scanResults from "./screens/scan-results.js";

const root = document.querySelector("#app");

const nav = {
  home: () => show(home),
  scanResults: (result) => show(scanResults, result),
};

function show(screen, ...args) {
  root.replaceChildren();
  window.scrollTo(0, 0);
  screen.render(root, nav, ...args);
}

nav.home();
