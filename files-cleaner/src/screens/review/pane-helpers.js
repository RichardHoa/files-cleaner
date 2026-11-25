// The panes reach each other only through the Review screen's context, never by importing one
// another, so pieces they share live here.
import { pathSeparator } from "../../shared/api.js";
import { el } from "../../shared/dom.js";
import { count, splitPath } from "../../shared/format.js";
import { icon } from "../../shared/icons.js";

export const FILES_SHOWN = 50;

export const fileName = (path) => splitPath(path, pathSeparator).name;

export const displayName = (choice) => choice.name.trim() || "No name";

export const UNSORTED = "Unsorted";

export const LEAVE_UNSORTED = "Leave unsorted";

export const trashedMark = () => el("span", { className: "folder-file-trashed", role: "img", ariaLabel: "Trashed", title: "Trashed" }, icon("trash"));

export const copiesText = (n) => `${count(n)} ${n === 1 ? "copy" : "copies"}`;
