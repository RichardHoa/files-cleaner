//! A Session's Journal read back: the state each Operation is in now.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::journal::{Journals, OutcomeKind, Phase, Record, SessionError, SessionHeader};
use crate::os_trash::TRASH_PLACE;
use crate::planner::{OperationKind, SourceState};
use crate::scanner::exists;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    /// Journaled as about to happen, with no outcome after: the Session
    /// stopped there.
    Started,
    /// Carried out, and not undone (or redone since).
    Done,
    Skipped,
    Failed,
    /// Carried out, then undone. Redo can carry it out again.
    Undone,
    /// Carried out, but the trashed copy is gone, or where it landed was
    /// never recorded.
    Unrecoverable,
}

/// One Operation as its Journal records tell it.
pub(crate) struct JournaledOperation {
    pub id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    /// `None` for a Trash that hasn't landed.
    pub destination: Option<PathBuf>,
    /// Whether `destination` came from the OS Trash.
    pub in_system_trash: bool,
    pub planned: SourceState,
    pub state: OperationState,
    /// Why it wasn't done, as journaled in its last outcome's `reason`.
    pub note: Option<String>,
    /// Set while the last intent for it has no outcome: that pass was cut
    /// short by a crash, maybe after the item moved.
    pub interrupted: Option<Phase>,
}

impl JournaledOperation {
    /// The record written before `phase` touches the item.
    pub fn intent(&self, phase: Phase) -> Record {
        match phase {
            Phase::Run => Record::Operation {
                operation_id: self.id.clone(),
                kind: self.kind,
                source: self.source.clone(),
                destination: self.destination.clone(),
                planned: self.planned,
            },
            Phase::Undo | Phase::Redo => Record::Intent {
                operation_id: self.id.clone(),
                phase,
            },
        }
    }

    /// Where `phase` moves the item from, and to, if that is known.
    pub fn route(&self, phase: Phase) -> Option<(&Path, &Path)> {
        let destination = self.destination.as_deref()?;
        Some(match phase {
            Phase::Run | Phase::Redo => (&self.source, destination),
            Phase::Undo => (destination, &self.source),
        })
    }

    /// True if a crash cut short sending it to the System Trash, it left its
    /// place, and where it landed was never recorded. A location from before
    /// an earlier Undo says nothing about this pass, so it isn't looked at:
    /// the app never guesses which item in the Trash is this one.
    pub fn may_be_in_trash(&self) -> bool {
        self.kind == OperationKind::Trash
            && matches!(self.interrupted, Some(Phase::Run | Phase::Redo))
            && !exists(&self.source)
            && (self.in_system_trash || self.destination.as_deref().is_none_or(|d| !exists(d)))
    }
}

/// The state an Operation is in once `phase` ended with `outcome`.
fn next_state(state: OperationState, phase: Phase, outcome: OutcomeKind) -> OperationState {
    match (phase, outcome) {
        (Phase::Run | Phase::Redo, OutcomeKind::Done) => OperationState::Done,
        (Phase::Undo, OutcomeKind::Done) => OperationState::Undone,
        (Phase::Run, OutcomeKind::Skipped) => OperationState::Skipped,
        (_, OutcomeKind::Failed) => OperationState::Failed,
        (_, OutcomeKind::Unrecoverable) => OperationState::Unrecoverable,
        // A skipped Undo or Redo leaves the Operation as it was.
        (Phase::Undo | Phase::Redo, OutcomeKind::Skipped) => state,
    }
}

/// The Note for an item whose location wasn't recorded.
pub(crate) fn restore_by_hand_note() -> String {
    format!("It may be in the {TRASH_PLACE}, restore it by hand")
}

/// A Session's Journal, read.
pub(crate) struct SessionJournal {
    pub header: SessionHeader,
    pub finished: bool,
    /// In the order they ran.
    pub operations: Vec<JournaledOperation>,
}

impl SessionJournal {
    pub fn read(journals: &Journals, id: &str) -> Result<SessionJournal, SessionError> {
        let (header, records) = journals.read(id)?;
        let mut session = SessionJournal {
            header,
            finished: false,
            operations: Vec::new(),
        };
        for record in &records {
            session.apply(record);
        }
        // Worked out on each read, not journaled: if the user restores the
        // item by hand, the Operation goes back to what it was.
        for op in &mut session.operations {
            if op.may_be_in_trash() {
                op.state = OperationState::Unrecoverable;
                op.note = Some(restore_by_hand_note());
            }
        }
        Ok(session)
    }

    /// Brings the Session up to date with `record`, as just read or written.
    pub fn apply(&mut self, record: &Record) {
        match record {
            Record::Operation {
                operation_id,
                kind,
                source,
                destination,
                planned,
            } => {
                let op = JournaledOperation {
                    id: operation_id.clone(),
                    kind: *kind,
                    source: source.clone(),
                    destination: destination.clone(),
                    in_system_trash: false,
                    planned: *planned,
                    state: OperationState::Started,
                    note: None,
                    interrupted: Some(Phase::Run),
                };
                // A retry runs an Operation of the Session again, in its place.
                match self.operation(operation_id) {
                    Some(existing) => *existing = op,
                    None => self.operations.push(op),
                }
            }
            Record::Intent {
                operation_id,
                phase,
            } => {
                if let Some(op) = self.operation(operation_id) {
                    op.interrupted = Some(*phase);
                }
            }
            Record::Outcome {
                operation_id,
                phase,
                outcome,
                reason: note,
                landed,
            } => {
                if let Some(op) = self.operation(operation_id) {
                    if landed.is_some() {
                        op.destination = landed.clone();
                        op.in_system_trash = true;
                    }
                    op.state = next_state(op.state, *phase, *outcome);
                    op.note = note.clone();
                    op.interrupted = None;
                }
            }
            Record::Finished => self.finished = true,
        }
    }

    /// The Operation `id`; a record about any other is ignored.
    fn operation(&mut self, id: &str) -> Option<&mut JournaledOperation> {
        self.operations.iter_mut().rev().find(|op| op.id == id)
    }

    /// The Operations in `state`, limited to `only` if given.
    pub fn in_state<'a>(
        &'a self,
        state: OperationState,
        only: Option<&'a HashSet<&str>>,
    ) -> impl DoubleEndedIterator<Item = &'a JournaledOperation> {
        self.operations.iter().filter(move |op| {
            op.state == state && only.is_none_or(|ids| ids.contains(op.id.as_str()))
        })
    }
}
