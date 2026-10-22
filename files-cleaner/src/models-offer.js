// The first-open offer of the Grouping Models: a sheet over Home, shown once. Its answer is remembered,
// so after "Not now" the offer only appears in the Suggested Folders step.
import {
  answerGroupingModelsOffer,
  describeError,
  downloadGroupingModels,
  groupingModelsOfferPending,
  groupingModelsStatus,
} from "./api.js";
import { el } from "./dom.js";
import { groupingModelsPitch } from "./models-panel.js";
import { showStatus } from "./status.js";

/** Shows the offer if it hasn't been answered yet. */
export async function offerGroupingModelsOnce() {
  let total;
  try {
    if (!(await groupingModelsOfferPending())) return;
    ({ total } = await groupingModelsStatus());
  } catch {
    // Asked again next time; the app works without it.
    return;
  }

  const download = el("button", { type: "button", className: "primary" }, "Download");
  const notNow = el("button", { type: "button" }, "Not now");
  const error = el("p", { className: "status error" });
  const sheet = el(
    "dialog",
    { className: "sheet" },
    el("h2", {}, "Group files by what's inside them?"),
    el("p", {}, groupingModelsPitch(total)),
    error,
    el("div", { className: "actions" }, notNow, download),
  );

  const answer = async (choice) => {
    try {
      await answerGroupingModelsOffer(choice);
    } catch {
      // Only the memory of the answer is lost: the offer shows again next time.
    }
    sheet.close();
    sheet.remove();
  };
  download.addEventListener("click", async () => {
    download.disabled = true;
    try {
      await downloadGroupingModels();
      await answer("download");
    } catch (e) {
      // Too little space, say: the user reads why and can still say "Not now".
      showStatus(error, describeError(e), true);
      download.disabled = false;
    }
  });
  notNow.addEventListener("click", () => answer("notNow"));
  // Escape counts as "Not now".
  sheet.addEventListener("cancel", (event) => {
    event.preventDefault();
    answer("notNow");
  });

  document.body.append(sheet);
  sheet.showModal();
}
