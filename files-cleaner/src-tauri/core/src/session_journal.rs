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
    /// No outcome was journaled: the Session stopped there.
    Started,
    Done,
    Skipped,
    Failed,
    Undone,
    /// The trashed copy is gone, or where it landed was never recorded.
    Unrecoverable,
}

pub(crate) struct JournaledOperation {
    pub id: String,
    pub kind: OperationKind,
    pub source: PathBuf,
    /// `None` for a Trash that hasn't landed.
    pub destination: Option<PathBuf>,
    pub in_system_trash: bool,
    pub planned: SourceState,
    pub state: OperationState,
    pub note: Option<String>,
    /// A crash cut this pass short, maybe after the item moved.
    pub interrupted: Option<Phase>,
}

impl JournaledOperation {
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

    pub fn route(&self, phase: Phase) -> Option<(&Path, &Path)> {
        let destination = self.destination.as_deref()?;
        Some(match phase {
            Phase::Run | Phase::Redo => (&self.source, destination),
            Phase::Undo => (destination, &self.source),
        })
    }

    /// A landing from before an earlier Undo says nothing about this pass, so
    /// the app never guesses which item in the Trash is this one.
    pub fn may_be_in_trash(&self) -> bool {
        self.kind == OperationKind::Trash
            && matches!(self.interrupted, Some(Phase::Run | Phase::Redo))
            && !exists(&self.source)
            && (self.in_system_trash || self.destination.as_deref().is_none_or(|d| !exists(d)))
    }
}

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

pub(crate) fn restore_by_hand_note() -> String {
    format!("It may be in the {TRASH_PLACE}, restore it by hand")
}

pub(crate) struct SessionJournal {
    pub header: SessionHeader,
    pub finished: bool,
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
        // Not journaled: if the user restores the item by hand, the Operation
        // goes back to what it was.
        for op in &mut session.operations {
            if op.may_be_in_trash() {
                op.state = OperationState::Unrecoverable;
                op.note = Some(restore_by_hand_note());
            }
        }
        Ok(session)
    }

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

    fn operation(&mut self, id: &str) -> Option<&mut JournaledOperation> {
        self.operations.iter_mut().rev().find(|op| op.id == id)
    }

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
