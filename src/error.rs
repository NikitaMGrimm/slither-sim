//! Machine-readable causes for core and session failures. Display text is diagnostic only.
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Stable numeric cause codes. Zero is reserved for no error; 18 and 19 for C boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum ErrorCode {
    InvalidRequest = 1,
    InvalidConfig = 2,
    ActionShape = 3,
    InvalidAction = 4,
    UnknownSlot = 5,
    UnknownWorld = 6,
    UnknownEntity = 7,
    DuplicateId = 8,
    InvalidState = 9,
    PlacementExhausted = 10,
    NotReset = 11,
    WorldStopped = 12,
    UnsupportedCheckpoint = 13,
    InvalidCheckpoint = 14,
    NumericOverflow = 15,
    RepresentationLimit = 16,
    AllocationFailed = 17,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidConfig => "invalid_config",
            Self::ActionShape => "action_shape",
            Self::InvalidAction => "invalid_action",
            Self::UnknownSlot => "unknown_slot",
            Self::UnknownWorld => "unknown_world",
            Self::UnknownEntity => "unknown_entity",
            Self::DuplicateId => "duplicate_id",
            Self::InvalidState => "invalid_state",
            Self::PlacementExhausted => "placement_exhausted",
            Self::NotReset => "not_reset",
            Self::WorldStopped => "world_stopped",
            Self::UnsupportedCheckpoint => "unsupported_checkpoint",
            Self::InvalidCheckpoint => "invalid_checkpoint",
            Self::NumericOverflow => "numeric_overflow",
            Self::RepresentationLimit => "representation_limit",
            Self::AllocationFailed => "allocation_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Constraint {
    Missing,
    NonFinite,
    Negative,
    NonPositive,
    OutOfRange,
    LengthMismatch,
    Duplicate,
    Ordering,
    OutsideArena,
    InconsistentState,
}

impl Constraint {
    pub(crate) fn positive(value: f64) -> Option<Self> {
        if !value.is_finite() {
            Some(Self::NonFinite)
        } else if value <= 0.0 {
            Some(Self::NonPositive)
        } else {
            None
        }
    }
    pub(crate) fn nonnegative(value: f64) -> Option<Self> {
        if !value.is_finite() {
            Some(Self::NonFinite)
        } else if value < 0.0 {
            Some(Self::Negative)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
pub enum ErrorKind {
    InvalidRequest,
    InvalidConfig {
        rule: Constraint,
    },
    ActionShape {
        expected: usize,
        actual: usize,
    },
    InvalidAction {
        rule: Constraint,
    },
    UnknownSlot,
    UnknownWorld,
    UnknownEntity,
    DuplicateId,
    InvalidState {
        rule: Constraint,
    },
    PlacementExhausted {
        attempts: u32,
    },
    NotReset,
    WorldStopped,
    UnsupportedCheckpoint {
        format: String,
        rules_revision: String,
    },
    InvalidCheckpoint {
        rule: Constraint,
    },
    NumericOverflow,
    RepresentationLimit,
    AllocationFailed,
}

/// A cause plus optional zero-based world/slot and field location.
/// Strings are constructed only on failure, never on successful ticks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimError {
    pub kind: ErrorKind,
    pub world: Option<usize>,
    pub slot: Option<usize>,
    pub field: Option<String>,
    message: Box<Diagnostic>,
}

// Keep the diagnostic off the inline payload so causes and step errors can be
// returned by value. Only constructing/formatting an error uses this allocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Diagnostic(String);

impl SimError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            world: None,
            slot: None,
            field: None,
            message: Box::new(Diagnostic(message.into())),
        }
    }
    pub fn at_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
    pub fn at_slot(mut self, slot: usize) -> Self {
        self.slot = Some(slot);
        self
    }
    pub fn at_world(mut self, world: usize) -> Self {
        self.world = Some(world);
        self
    }
    pub const fn code(&self) -> ErrorCode {
        match self.kind {
            ErrorKind::InvalidRequest => ErrorCode::InvalidRequest,
            ErrorKind::InvalidConfig { .. } => ErrorCode::InvalidConfig,
            ErrorKind::ActionShape { .. } => ErrorCode::ActionShape,
            ErrorKind::InvalidAction { .. } => ErrorCode::InvalidAction,
            ErrorKind::UnknownSlot => ErrorCode::UnknownSlot,
            ErrorKind::UnknownWorld => ErrorCode::UnknownWorld,
            ErrorKind::UnknownEntity => ErrorCode::UnknownEntity,
            ErrorKind::DuplicateId => ErrorCode::DuplicateId,
            ErrorKind::InvalidState { .. } => ErrorCode::InvalidState,
            ErrorKind::PlacementExhausted { .. } => ErrorCode::PlacementExhausted,
            ErrorKind::NotReset => ErrorCode::NotReset,
            ErrorKind::WorldStopped => ErrorCode::WorldStopped,
            ErrorKind::UnsupportedCheckpoint { .. } => ErrorCode::UnsupportedCheckpoint,
            ErrorKind::InvalidCheckpoint { .. } => ErrorCode::InvalidCheckpoint,
            ErrorKind::NumericOverflow => ErrorCode::NumericOverflow,
            ErrorKind::RepresentationLimit => ErrorCode::RepresentationLimit,
            ErrorKind::AllocationFailed => ErrorCode::AllocationFailed,
        }
    }
    pub(crate) fn config(rule: Constraint, field: &str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidConfig { rule }, message).at_field(field)
    }
    pub(crate) fn state(rule: Constraint, field: &str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidState { rule }, message).at_field(field)
    }
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(world) = self.world {
            write!(f, "World {world}: ")?;
        }
        f.write_str(&self.message.0)
    }
}
impl Error for SimError {}

/// Rejection preserves the world; stopping completes the tick and persists its cause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreStepError {
    Rejected(SimError),
    Stopped(SimError),
}
impl fmt::Display for CoreStepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(cause) | Self::Stopped(cause) => fmt::Display::fmt(cause, f),
        }
    }
}
impl Error for CoreStepError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Rejected(cause) | Self::Stopped(cause) => cause,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldStatus<'a> {
    Running,
    Stopped(&'a SimError),
}
