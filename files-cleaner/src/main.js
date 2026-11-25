// Review is kept while another screen shows, so View › Review can go back to it unchanged.
import * as history from "./screens/history/history.js";
import * as home from "./screens/home/home.js";
import * as review from "./screens/review/review.js";
import * as rules from "./screens/rules/rules.js";
import * as session from "./screens/session/session.js";
import * as settings from "./screens/settings/settings.js";
import { cancelAnalysis, findDuplicates, pathSeparator, recentScanRoots, scan } from "./shared/api.js";
import { splitPath } from "./shared/format.js";
import { refreshMenu, screenMenu, setAppMenu, setScreenMenu } from "./shared/menus/menus.js";
import { showIntroduction, showIntroductionOnce } from "./shared/onboarding/introduction.js";
import { stepFlow } from "./steps.js";

const root = document.querySelector("#app");

// `{ nodes, menu }` of the Review screen, while it can be gone back to.
let kept = null;
let reviewShown = false;

const nav = {
  /** `start` comes from the menu bar: `{ pick: true }` or `{ folder }`. */
  home: (start = null) => show(home, start),
  review: (result, duplicates) => {
    dropKeptReview();
    show(review, result, duplicates);
    reviewShown = true;
    kept = { nodes: null, menu: null };
  },
  backToReview: () => {
    if (!kept || reviewShown) return;
    root.replaceChildren(...kept.nodes);
    reviewShown = true;
    setScreenMenu(kept.menu);
  },
  leaveReview: () => {
    kept = null;
    reviewShown = false;
  },
  history: (page) => show(history, page),
  /** `from` is the History page to go Back to; `result` an action's `{ summary, action, done }`. */
  session: (sessionId, from = null, result = null) => show(session, sessionId, from, result),
  settings: () => show(settings),
  /** `from` says where Back returns. */
  rules: (from = "home") => show(rules, from),
};
// A new scan ends the Review kept, and the reading it may still be doing.
nav.steps = stepFlow({ findDuplicates, scan: (...args) => (dropKeptReview(), scan(...args)) }, nav);

/** Its reading would otherwise go on unseen. */
function dropKeptReview() {
  if (kept && !reviewShown) cancelAnalysis().catch(() => {});
  nav.leaveReview();
}

function show(screen, ...args) {
  if (reviewShown) kept = { nodes: [...root.childNodes], menu: screenMenu() };
  reviewShown = false;
  setScreenMenu(null);
  root.replaceChildren();
  window.scrollTo(0, 0);
  screen.render(root, nav, ...args);
  showRecent();
}

// Read again whenever the screen changes, as any scan may have changed it.
let recent = [];
async function showRecent() {
  try {
    recent = (await recentScanRoots()).filter((folder) => folder.exists);
  } catch {
    recent = [];
  }
  refreshMenu();
}

setAppMenu(() => ({
  newScan: () => nav.home({ pick: true }),
  recent: recent.map(({ path }) => ({ label: splitPath(path, pathSeparator).name, action: () => nav.home({ folder: path }) })),
  review: kept && !reviewShown ? nav.backToReview : null,
  rules: () => nav.rules(),
  history: () => nav.history(1),
  settings: () => nav.settings(),
  help: showIntroduction,
}));

nav.home();
showIntroductionOnce();
