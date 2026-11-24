// Shows one screen at a time. Each screen module exports `render(root, nav, ...args)`
// and builds its own DOM inside `root`. The Review screen is kept while another is shown from the menu bar,
// so View › Review goes back to it, until it's left for good or another scan starts.
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

// The Review screen while it can be gone back to: `{ nodes, menu }`, its page and its menu bar items, and
// whether it's shown now.
let kept = null;
let reviewShown = false;

const nav = {
  /** `start` is `{ pick: true }` to pick a folder to scan at once, or `{ folder }` to scan it. */
  home: (start = null) => show(home, start),
  review: (result, duplicates) => {
    dropKeptReview();
    show(review, result, duplicates);
    reviewShown = true;
    kept = { nodes: null, menu: null };
  },
  /** Goes back to the Review screen kept, as it was left. */
  backToReview: () => {
    if (!kept || reviewShown) return;
    root.replaceChildren(...kept.nodes);
    reviewShown = true;
    setScreenMenu(kept.menu);
  },
  /** Forgets the Review screen, left by its Cancel or its Run: nothing goes back to it. */
  leaveReview: () => {
    kept = null;
    reviewShown = false;
  },
  history: (page) => show(history, page),
  /**
   * `from` is `{ page }` when opened from that History page, so Back returns there; `result`
   * opens it with an action's result, `{ summary, action, done }` as for `describeWarning`, as after running a Plan.
   */
  session: (sessionId, from = null, result = null) => show(session, sessionId, from, result),
  settings: () => show(settings),
  /** `from` is "settings" when opened from Settings, so Back returns there. */
  rules: (from = "home") => show(rules, from),
};
// A new scan ends the Review kept, and the reading it may still be doing.
nav.steps = stepFlow({ findDuplicates, scan: (...args) => (dropKeptReview(), scan(...args)) }, nav);

/** Forgets the Review screen kept while another is shown, stopping its reading. */
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

// The Recent Scan Roots, for File › Scan Recent, read again whenever the screen changes.
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
