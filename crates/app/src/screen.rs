//! The screens Kabar can show, their names and where each sits in the navigation.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Today,
    Journey,
    Reader,
    Review,
    Coach,
    Play,
    Studio,
    Settings,
    PenLab,
    Devices,
    /// The Lesson Player, full screen; the lesson is `Ui::lesson`.
    Lesson,
    /// Teach Kabar a word (spec §26).
    Teach,
    /// The learning journal: the timeline and reflections (spec §6).
    Journal,
    /// Cost & limits (spec §19), from Settings.
    Cost,
    /// Stats (spec §16): from the Review home and Today.
    Stats,
    /// The monthly level check (spec §6).
    LevelCheck,
    /// Sinetron and Kabar Radio.
    Listen,
    /// Persona chats (spec §15).
    Chat,
}

impl Screen {
    pub fn label(self) -> Msg {
        match self {
            Self::Today => Msg::NavToday,
            Self::Journey => Msg::NavJourney,
            Self::Reader => Msg::NavReader,
            Self::Review => Msg::NavReview,
            Self::Coach => Msg::NavCoach,
            Self::Play => Msg::NavPlay,
            Self::Studio => Msg::NavStudio,
            Self::Settings => Msg::NavSettings,
            Self::PenLab => Msg::NavPenLab,
            Self::Devices => Msg::NavDevices,
            Self::Lesson => Msg::NavJourney,
            Self::Teach => Msg::NavTeach,
            Self::Journal => Msg::NavJournal,
            Self::Cost => Msg::CostTitle,
            Self::Stats => Msg::NavStats,
            Self::LevelCheck => Msg::NavCoach,
            Self::Listen => Msg::NavListen,
            Self::Chat => Msg::NavChat,
        }
    }

    /// Hidden from the phone's bottom bar, which keeps five items within
    /// thumb reach. Kak Nur, the journal, Watch & listen, Chat and Teach
    /// Kabar are linked from Today there (the journal from Kak Nur too); Settings is the gear
    /// in the phone's top bar.
    /// Pen tools: shown on a tablet only (`Ui::tablet`).
    pub fn pen_only(self) -> bool {
        matches!(self, Self::Studio | Self::PenLab)
    }

    pub(crate) fn secondary(self) -> bool {
        matches!(
            self,
            Self::Play
                | Self::Coach
                | Self::Journal
                | Self::Settings
                | Self::Studio
                | Self::PenLab
                | Self::Devices
                | Self::Teach
                | Self::Cost
                | Self::Listen
                | Self::Chat
                | Self::Stats
        )
    }
}
