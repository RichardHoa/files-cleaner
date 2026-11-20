// The Review screen's reading: the loose files are read in the background, which needs the Grouping Models
// (without them, they're offered in the Suggested Folders' place, or the user's Rules alone sort the files), with a counter, an estimate and a wait
// note, and how far it is on the sidebar. The Rules' folders need no reading, so they're shown at once, and the
// other Suggested Folders are added once that's done. The reading can be stopped early to group only what was
// read; Cancel stops it and goes Home.
import {
  analyseLooseFiles,
  answerNotifications,
  cancelAnalysis,
  describeError,
  groupingModelsStatus,
  loadConfig,
  notifications,
  notifyInBackground,
  readingPace,
  ruleFolders,
  saveReadingPace,
  stopReading,
} from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { count, plural } from "../../shared/format.js";
import { groupingModelsPanel } from "../../shared/grouping-models/models-panel.js";
import { warningBlock } from "../../shared/outcomes.js";
import { progressBar } from "../../shared/progress-bar.js";
import { showStatus } from "../../shared/status.js";
import { describeEstimate, readingEstimate, updatedAverage } from "./reading-estimate.js";

/** What the reading screen says about the wait. */
const WAIT_TEXT = "This takes a while. Feel free to switch apps.";
/** What the reading estimate calls this computer. */
const COMPUTER = /Mac/.test(navigator.userAgent) ? "Mac" : "computer";

/**
 * The analysis's `{ finished, total, current, phase }` progress as "Reading 12 of 85 files…", or what comes
 * before and after, then the file it is on, or that the models are loading.
 */
const describeAnalysisProgress = ({ finished, total, current, phase }) =>
  ({
    checking: () => `Checking which files were read before: ${count(finished)} of ${count(total)}…\n${current}`,
    loading: () => `Reading 0 of ${plural(total, "file")}…\nLoading the grouping models…`,
    reading: () => `Reading ${count(finished)} of ${plural(total, "file")}…\n${current}`,
    placing: () => `Finding folders for files left out: ${count(finished)} of ${count(total)}…\n${current}`,
  })[phase]();

/**
 * The line saying how long a file takes to read on this computer and how long until the folders are shown,
 * from the stored pace and the analysis's progress, kept counting down each second. `save()` stores this
 * run's pace once it has finished; `stop()` stops the countdown either way.
 */
function estimateLine() {
  const node = el("p", { className: "reading-estimate muted" });
  const started = performance.now();
  const elapsed = () => (performance.now() - started) / 1000;
  let average = null;
  // The files that need reading, and how many are read; the last pass is counted within the time per file.
  let reading = null;
  const draw = () => {
    node.textContent =
      reading?.total > 0
        ? describeEstimate(readingEstimate({ average, total: reading.total, finished: reading.finished, elapsed: elapsed() }), COMPUTER)
        : "";
  };
  readingPace()
    .then((pace) => {
      average = pace;
      draw();
    })
    // No history to go by: the estimate starts from the first guess.
    .catch(() => {});
  const ticking = setInterval(draw, 1000);
  return {
    node,
    update(progress) {
      if (progress.phase === "loading" || progress.phase === "reading") reading = progress;
      draw();
    },
    save() {
      const files = reading?.total ?? 0;
      // Not saved, the next estimate is only less close.
      if (files > 0) saveReadingPace(updatedAverage(average, { seconds: elapsed(), files })).catch(() => {});
    },
    stop: () => clearInterval(ticking),
  };
}

/**
 * The wait text, and the first time, a question whether to notify the user when the folders are ready. The
 * answer is kept in the config, so it's asked only once.
 */
function waitNote() {
  const text = el("p", { className: "note" }, WAIT_TEXT);
  const ask = el("p", { className: "notify-ask", hidden: true });
  const answer = (wanted) => {
    ask.hidden = true;
    // Not saved, the question comes again next time.
    answerNotifications(wanted).catch(() => {});
  };
  const yes = el("button", { type: "button", className: "small" }, "Notify me");
  const no = el("button", { type: "button", className: "small" }, "Don't notify");
  yes.addEventListener("click", () => answer(true));
  no.addEventListener("click", () => answer(false));
  ask.append(el("span", {}, "Notify you when the folders are ready?"), yes, no);
  notifications()
    .then((wanted) => {
      ask.hidden = wanted !== null;
    })
    // The config can't be read: no question, and nothing is sent.
    .catch(() => {});
  return [text, ask];
}

/**
 * The reading of the Review screen with context `ctx`: `waiting()`, what the Suggested Folders' pane shows
 * until they're ready, `footer`, saying where the files were read, `start()` and `cancel()`.
 */
export function reading(ctx) {
  const { nav, state, view } = ctx;
  const footer = el("div");
  // Until the Suggested Folders are ready, their pane shows `waiting`: the reading, the offer or the error.
  let waiting = [el("h2", {}, "Suggested Folders"), el("p", { className: "status" }, "Reading the files…")];
  // Set while the files are being read, and once Cancel was clicked then: the screen goes Home once
  // the reading has stopped. Once Stop reading was clicked, the files read so far are grouped instead.
  let analysing = false;
  let cancelled = false;
  let stopped = false;

  /**
   * Goes Home, once the reading has stopped if the files are being read, so it isn't still running when the
   * user moves on. Returns whether it's waiting for that.
   */
  function cancel() {
    if (!analysing) {
      nav.home();
      return false;
    }
    cancelled = true;
    // Said under the header's button, beside Cancel.
    const { status } = ctx.sessions;
    showStatus(status, "Stopping…");
    cancelAnalysis().catch((error) => showStatus(status, describeError(error), true));
    return true;
  }

  /** Shows `nodes` in the Suggested Folders' pane until they're ready, with `phase` as their phase. */
  function waitFor(phase, nodes) {
    state.waitFor(phase);
    if (phase !== "reading") ctx.sidebar.showProgress("");
    waiting = [el("h2", {}, "Suggested Folders"), ...nodes];
    ctx.redraw.section("folders");
  }

  /**
   * Reads the files, with a counter, the time a file takes on this computer and a countdown to the folders,
   * while the rest of the Plan can be reviewed. A finished run updates that time; the user is notified when
   * it ends while the app is in the background.
   */
  async function analyse() {
    const progress = el("p", { className: "status" }, "Reading the files…");
    const bar = progressBar();
    const estimate = estimateLine();
    const stop = el("button", { type: "button", className: "small" }, "Stop reading");
    stop.addEventListener("click", () => {
      stopped = true;
      stop.disabled = true;
      showStatus(progress, "Stopping after this file…");
      stopReading().catch((error) => showStatus(progress, describeError(error), true));
    });
    waitFor("reading", [progress, bar.node, estimate.node, el("p", {}, stop), ...waitNote()]);
    analysing = true;
    try {
      // Not shown, the Rules' folders come with the rest.
      const ruled = await ruleFolders().catch(() => null);
      if (ruled?.folders.length > 0 && !cancelled) {
        state.showRuled(ruled);
        showFolders(state.choices);
      }
      const analysis = await analyseLooseFiles((update) => {
        if (cancelled || stopped) return;
        // Every file is read by the last pass: stopping then would only skip it.
        if (update.phase === "placing") stop.hidden = true;
        showStatus(progress, describeAnalysisProgress(update));
        // Moves back and forth while the models load, as nothing is read meanwhile.
        bar.set(update.finished, update.phase === "loading" ? 0 : update.total);
        estimate.update(update);
        ctx.sidebar.showProgress(update.phase === "reading" && update.total > 0 ? `${Math.floor((100 * update.finished) / update.total)}%` : "");
      });
      if (cancelled) return nav.home();
      // A stopped run says nothing of this computer's pace, and the user is there to see it end.
      if (!stopped) {
        estimate.save();
        notifyInBackground(`Folders are ready for ${ctx.rootName}`).catch(() => {});
      }
      ready(analysis);
    } catch (error) {
      if (cancelled) return nav.home();
      if (!stopped) notifyInBackground(`Couldn't finish reading ${ctx.rootName}`).catch(() => {});
      waitFor("failed", [warningBlock(describeError(error))]);
    } finally {
      analysing = false;
      estimate.stop();
    }
  }

  /** Lists the first of `added`, the Suggested Folders just filled in, if the pane shows the reading; else brings the screen up to date. */
  function showFolders(added) {
    if (view.current === "folders") ctx.show(added[0] ?? state.choices[0] ?? "left");
    else ctx.redraw.all();
  }

  /** Fills in the Suggested Folders and the files left where they are, from the analysis, with the choices made on the copies meanwhile. */
  function ready(analysis) {
    const added = state.ready(analysis.grouping);
    if (analysis.device) footer.replaceChildren(el("p", { className: "note" }, `Files read on ${analysis.device}.`));
    showFolders(added);
  }

  /** Reads the files if the Grouping Models are installed; without them, sorts by the Rules alone if there are any, or else offers the models. */
  async function start() {
    try {
      const models = await groupingModelsStatus();
      if (models.state === "installed") analyse();
      else if ((await loadConfig()).config.rules.some((rule) => rule.enabled)) {
        // Nothing is read: the Rules' folders come back at once.
        ready(await analyseLooseFiles(() => {}));
        footer.replaceChildren(el("p", { className: "note" }, "Sorted by your Rules only."));
      } else {
        waitFor("offer", [groupingModelsPanel(analyse)]);
      }
    } catch (error) {
      waitFor("failed", [el("p", { className: "status error" }, describeError(error))]);
    }
  }

  return { waiting: () => waiting, footer, start, cancel };
}
