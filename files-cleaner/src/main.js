// Shows one screen at a time. Each screen module exports `render(root, nav, ...args)`
// and builds its own DOM inside `root`.
import * as history from "./screens/history.js";
import * as home from "./screens/home.js";
import * as review from "./screens/review.js";
import * as session from "./screens/session.js";
import * as settings from "./screens/settings.js";
import { findDuplicates, scan } from "./api.js";
import { offerGroupingModelsOnce } from "./models-offer.js";
import { stepFlow } from "./steps.js";

const root = document.querySelector("#app");

const nav = {
  home: () => show(home),
  review: (result, duplicates) => show(review, result, duplicates),
  history: (page) => show(history, page),
  /**
   * `from` is `{ page }` when opened from that History page, so Back returns there; `message` opens
   * it with that status line, as after running a Plan.
   */
  session: (sessionId, from = null, message = null) => show(session, sessionId, from, message),
  settings: () => show(settings),
};
nav.steps = stepFlow({ findDuplicates, scan }, nav);

function show(screen, ...args) {
  root.replaceChildren();
  window.scrollTo(0, 0);
  screen.render(root, nav, ...args);
}

nav.home();
offerGroupingModelsOnce();
