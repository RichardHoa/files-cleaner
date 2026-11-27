// Read in the background, as it is slow; the Rules' folders need no reading, so they show at once.
// A long read asks first.
import {
  analyseLooseFiles,
  answerNotifications,
  cancelAnalysis,
  describeError,
  filesToRead,
  groupingModelsStatus,
  loadConfig,
  notifications,
  notifyInBackground,
  readingPace,
  ruleFolders,
  saveReadingPace,
  stopReading,
} from "../../shared/api.js";
import { el, progressBar, showStatus } from "../../shared/dom.js";
import { count, plural } from "../../shared/format.js";
import { groupingModelsPanel, thisComputer } from "../../shared/models-panel.js";
import { warningBlock } from "../../shared/outcomes.js";
import { foldersNote } from "./folder-panes.js";

const WAIT_TEXT = "This takes a while. Feel free to switch apps.";
const COMPUTER = thisComputer();

const describeAnalysisProgress = ({ finished, total, current, phase }) =>
  ({
    checking: () => `Checking which files were read before: ${count(finished)} of ${count(total)}…\n${current}`,
    loading: () => `Reading 0 of ${plural(total, "file")}…\nLoading the grouping models…`,
    reading: () => `Reading ${count(finished)} of ${plural(total, "file")}…\n${current}`,
    placing: () => `Finding folders for files left out: ${count(finished)} of ${count(total)}…\n${current}`,
  })[phase]();

/** Rounded roughly: only used for times over 10 minutes. */
function aboutTime(seconds) {
  const minutes = Math.round(seconds / 60);
  return minutes < 90 ? `about ${minutes} min` : `about ${Math.round(minutes / 60)} h`;
}

/** Counts down by itself each second, as progress only comes once per file. */
function estimateLine() {
  const node = el("p", { className: "reading-estimate muted" });
  const started = performance.now();
  const elapsed = () => (performance.now() - started) / 1000;
  let average = null;
  // The last pass is already counted within the time per file.
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

/** Asked only once: the answer is kept in the config. */
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

export function reading(ctx) {
  const { nav, state, view } = ctx;
  const footer = el("div");
  let waiting = [el("h2", {}, "Suggested Folders"), el("p", { className: "pane-note muted" }, foldersNote(ctx.rootName)), el("p", { className: "status" }, "Reading the files…")];
  // Cancel waits for the reading to stop before going Home; Stop groups what was read.
  let analysing = false;
  let cancelled = false;
  let stopped = false;

  /** Waits for the reading to stop, so it isn't still running when the user moves on. */
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

  function waitFor(phase, nodes) {
    state.waitFor(phase);
    if (phase !== "reading") ctx.sidebar.showProgress("");
    waiting = [el("h2", {}, "Suggested Folders"), el("p", { className: "pane-note muted" }, foldersNote(ctx.rootName)), ...nodes];
    ctx.redraw.section("folders");
  }

  /** In the background, so the rest of the Plan can be reviewed meanwhile. */
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
        // Nothing is read while the models load, so there is nothing to measure.
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

  function showFolders(added) {
    if (view.current === "folders") ctx.show(added[0] ?? state.choices[0] ?? "left");
    else ctx.redraw.all();
  }

  /** Choices made on copies while reading must survive. */
  function ready(analysis) {
    const added = state.ready(analysis.grouping);
    if (analysis.device) footer.replaceChildren(el("p", { className: "note" }, `Files read on ${analysis.device}.`));
    showFolders(added);
  }

  /** Over 10 minutes of reading should be the user's own decision. */
  async function readOrAsk() {
    waitFor("reading", [el("p", { className: "status" }, "Checking which files were read before…")]);
    analysing = true;
    let toRead;
    let average;
    // Both while `analysing`, so a Cancel meanwhile doesn't leave the reading to start.
    try {
      toRead = await filesToRead();
      // No history to go by: the first guess.
      average = await readingPace().catch(() => null);
    } catch (error) {
      if (cancelled) return nav.home();
      return waitFor("failed", [warningBlock(describeError(error))]);
    } finally {
      analysing = false;
    }
    if (cancelled) return nav.home();
    if (!readingNeedsAsking({ average, toRead })) return analyse();
    const read = el("button", { type: "button", className: "primary" }, "Read");
    const skip = el("button", { type: "button" }, "Don't read");
    read.addEventListener("click", () => analyse());
    skip.addEventListener("click", () => groupWithoutReading());
    const time = aboutTime(readingTime({ average, toRead }));
    waitFor("ask", [el("p", {}, `Reading ${plural(toRead, "file")} takes ${time} on this ${COMPUTER}`), el("div", { className: "actions" }, read, skip)]);
  }

  async function groupWithoutReading() {
    waitFor("reading", [el("p", { className: "status" }, "Sorting the files…")]);
    analysing = true;
    try {
      const analysis = await analyseLooseFiles(() => {}, { read: false });
      if (cancelled) return nav.home();
      ready(analysis);
      const read = el("button", { type: "button", className: "small" }, "Read files");
      read.addEventListener("click", () => {
        footer.replaceChildren();
        analyse();
      });
      footer.replaceChildren(el("p", { className: "note" }, "Files not read. ", read));
    } catch (error) {
      if (cancelled) return nav.home();
      waitFor("failed", [warningBlock(describeError(error))]);
    } finally {
      analysing = false;
    }
  }

  /** Without the models, Rules alone can still sort files. */
  async function start() {
    try {
      const models = await groupingModelsStatus();
      if (models.state === "installed") readOrAsk();
      else if ((await loadConfig()).config.rules.some((rule) => rule.enabled)) {
        // Nothing is read: the Rules' folders come back at once.
        ready(await analyseLooseFiles(() => {}));
        footer.replaceChildren(el("p", { className: "note" }, "Sorted by your Rules only."));
      } else {
        waitFor("offer", [groupingModelsPanel(readOrAsk)]);
      }
    } catch (error) {
      waitFor("failed", [el("p", { className: "status error" }, describeError(error))]);
    }
  }

  return { waiting: () => waiting, footer, start, cancel };
}

// Seconds per file is a run's whole time over the files it read, so loading the models and the last
// pass count too.

const FIRST_GUESS = 4;
/** In files. Before this many, this run's own pace isn't blended in. */
const SETTLED = 5;
const LONG_READ = 10 * 60;

/**
 * The stored average weighs as much as the first few files, so the estimate doesn't jump. It
 * counts down past the last file, to when the folders are shown.
 */
export function readingEstimate({ average, total, finished, elapsed }) {
  const stored = average ?? FIRST_GUESS;
  const perFile = finished < SETTLED ? stored : (stored * SETTLED + elapsed) / (SETTLED + finished);
  return { perFile, left: Math.max(0, perFile * total - elapsed) };
}

export function readingTime({ average, toRead }) {
  return (average ?? FIRST_GUESS) * toRead;
}

export function readingNeedsAsking({ average, toRead }) {
  return readingTime({ average, toRead }) > LONG_READ;
}

/** Each run counts for half, so the average follows this computer as it changes. */
export function updatedAverage(average, { seconds, files }) {
  if (files === 0) return average;
  const pace = seconds / files;
  return average == null ? pace : (average + pace) / 2;
}

export function describeEstimate({ perFile, left }, computer) {
  const seconds = Math.max(1, Math.round(perFile));
  const minutes = Math.round(left / 60);
  const remaining = minutes < 1 ? "less than a minute left" : `about ${minutes} min left`;
  return `About ${seconds} s per file on this ${computer} · ${remaining}`;
}
