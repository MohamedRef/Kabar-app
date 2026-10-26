//! Onboarding: why, minutes per day, a target, study times, reminder style,
//! then where the journey starts.

use kabar_protocol::coach::{CoachProfile, LocalDate, PlanSize, StartingPoint, Target};

/// The onboarding screens, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Name,
    Why,
    Focus,
    Minutes,
    Target,
    Times,
    Reminders,
    Start,
    Done,
}

impl Step {
    pub const ALL: [Self; 9] = [
        Self::Name,
        Self::Why,
        Self::Focus,
        Self::Minutes,
        Self::Target,
        Self::Times,
        Self::Reminders,
        Self::Start,
        Self::Done,
    ];

    pub fn next(self) -> Self {
        let _ = self;
        todo!("coach agent A")
    }

    pub fn prev(self) -> Self {
        let _ = self;
        todo!("coach agent A")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    NoStudyTime,
    TargetInPast,
    UnknownLevel(String),
}

/// Checks a finished profile; sorts and dedups study times.
pub fn finish(profile: CoachProfile, today: LocalDate) -> Result<CoachProfile, ProfileError> {
    let _ = (profile, today);
    todo!("coach agent A")
}

/// A realistic default target for this pace and starting point.
pub fn suggest_target(start: StartingPoint, size: PlanSize, today: LocalDate) -> Target {
    let _ = (start, size, today);
    todo!("coach agent A")
}

/// The first unit of the journey for a starting point (refined by the M8
/// placement test).
pub fn starting_unit(start: StartingPoint) -> &'static str {
    let _ = start;
    todo!("coach agent A")
}
