// Shows one screen at a time. Each screen module exports `render(root, nav, ...args)`
// and builds its own DOM inside `root`.
import * as history from "./screens/history/history.js";
import * as home from "./screens/home/home.js";
import * as review from "./screens/review/review.js";
import * as session from "./screens/session/session.js";
import * as settings from "./screens/settings/settings.js";
import { findDuplicates, scan } from "./shared/api.js";
import { showIntroductionOnce } from "./shared/onboarding/introduction.js";
import { stepFlow } from "./steps.js";

const root = document.querySelector("#app");

const nav = {
  home: () => show(home),
  review: (result, duplicates) => show(review, result, duplicates),
  history: (page) => show(history, page),
  /**
   * `from` is `{ page }` when opened from that History page, so Back returns there; `result`
   * opens it with an action's result, `{ summary, action, done }` as for `describeWarning`, as after running a Plan.
   */
  session: (sessionId, from = null, result = null) => show(session, sessionId, from, result),
  settings: () => show(settings),
};
nav.steps = stepFlow({ findDuplicates, scan }, nav);

function show(screen, ...args) {
  root.replaceChildren();
  window.scrollTo(0, 0);
  screen.render(root, nav, ...args);
}

nav.home();
showIntroductionOnce();
