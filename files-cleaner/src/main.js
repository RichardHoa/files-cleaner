// Shows one screen at a time. Each screen module exports `render(root, nav, ...args)`
// and builds its own DOM inside `root`.
import * as history from "./screens/history.js";
import * as home from "./screens/home.js";
import * as plan from "./screens/plan.js";
import * as scanResults from "./screens/scan-results.js";
import * as session from "./screens/session.js";

const root = document.querySelector("#app");

const nav = {
  home: () => show(home),
  scanResults: (result) => show(scanResults, result),
  plan: (result, built) => show(plan, result, built),
  history: (page) => show(history, page),
  /**
   * `from` is `{ page }` when opened from that History page, so Back returns there; `message`
   * opens it with that status line, as after running a Plan.
   */
  session: (sessionId, from = null, message = null) => show(session, sessionId, from, message),
};

function show(screen, ...args) {
  root.replaceChildren();
  window.scrollTo(0, 0);
  screen.render(root, nav, ...args);
}

nav.home();
