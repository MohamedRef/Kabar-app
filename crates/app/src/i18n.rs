//! UI strings in Egyptian Arabic, MSA and English. A typed catalog: a missing
//! translation is a compile error, not a blank button.
//!
//! Kak Nur's own sentences are not here: they are reviewed content rendered by
//! `kabar_core::coach::voice`. This file is the chrome around them.

use kabar_core::locale::UiLanguage;

/// Declares a fieldless string catalog and, for tests, `ALL`: every key, so
/// one test can walk every string in every language.
macro_rules! catalog {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($(#[$vm:meta])* $v:ident,)+ }) => {
        $(#[$m])*
        $vis enum $name {
            $($(#[$vm])* $v,)+
        }

        #[cfg(test)]
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$v),+];
        }
    };
}
pub(crate) use catalog;

catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    WelcomeKabarFrom,
    WelcomeExplain,
    WelcomeFirstWord,
    WelcomeStart,
    NavToday,
    NavJourney,
    NavReader,
    NavReview,
    NavCoach,
    NavStudio,
    NavSettings,
    NavGroupLearn,
    NavGroupPractise,
    NavGroupMe,
    NavPenLab,
    NavTeach,
    NavListen,
    NavChat,
    TodayTitle,
    ComingIn,
    ThemeSystem,
    ThemeSystemBlack,
    ThemeLight,
    ThemeDark,
    ThemeBlack,
    ThemeNight,
    ThemeBatik,
    ThemeSawah,
    CostTitle,
    CostUnitRequests,
    CostUnitCharacters,
    CostUnitNeurons,
    CostUnitRowsRead,
    CostUnitRowsWritten,
    CostPerDay,
    CostPerMonth,
    ExportTitle,
    ExportButton,
    ExportSaving,
    ExportSaved,
    ExportFailed,
    DiagTitle,
    DiagEmpty,
    DiagCount,
    DiagRefresh,
    DiagCopy,
    DiagCopied,
    ExportAndroidHint,
    PenLabClear,
    PenLabSave,
    PenLabCopy,
    PenLabCopied,
    PenLabSaved,
    PenLabEvents,
    PenLabSavedRuns,
    Width,
    WidthCompact,
    WidthMedium,
    WidthExpanded,
    WidthLarge,
    WidthExtraLarge,
    NavDevices,
    CloudNotConfigured,
    CloudSignIn,
    SyncLive,
    SyncConnecting,
    SyncCatchingUp,
    SyncOffline,
    SyncStopped,
    ProblemNotAllowed,
    ProblemSignInCancelled,
    ProblemSignInFailed,
    ProblemSignedOutRemotely,
    ProblemNetwork,
    ProblemSignInNetwork,
    ProblemServer,
    ProblemOutdated,
    DevicesRefresh,
    JourneyIndonesian,
    JourneyJavanese,
    JourneyRoute,
    JourneyUpNext,
    LessonLearn,
    LessonPractice,
    LessonReview,
    // Review (M6)
    RevSeedTitle,
    RevDue,
    RevNew,
    RevReviewedToday,
    RevRetention,
    RevForecastEmpty,
    RevAllDone,
    RevHowWell,
    RevContentUnchecked,
    RevContentChecked,
    RateAgain,
    RateHard,
    RateGood,
    RateEasy,
    RevPromptMeaning,
    RevPromptProduce,
    RevPromptListen,
    RevPromptCloze,
    RevPromptBridge,
    RevFromArabic,
    RevFalseFriend,
    RevUnverified,
    RevTapToReveal,
    RevDoneTitle,
    RevCards,
    RevRemembered,
    RevTime,
    RevStrongest,
    RevRevisit,
    RevBackHome,
    RevNothingHere,
    RevSaveFailed,
    RevUndoFailed,
    // --- coach: general ---
    Back,
    Next,
    Start,
    Undo,
    Dismiss,
    ShowMore,
    TodayWord,
    Yesterday,
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
    SkillReading,
    SkillListening,
    SkillWriting,
    SkillSpeaking,
    SkillVocabulary,
    WhyTravel,
    WhyWork,
    WhyFamily,
    WhyMedia,
    WhyFaith,
    WhyCuriosity,
    FocusBoth,
    OnbBothOrder,
    OnbNoTarget,
    OnbNoTargetChosen,
    OnbSetTarget,
    StartNew,
    StartNewHint,
    StartBasics,
    StartBasicsHint,
    StartChat,
    StartChatHint,
    OnbFinish,
    ErrNoStudyTime,
    ErrTargetInPast,
    ErrUnknownLevel,
    AddTime,
    RemoveTime,
    StyleGentle,
    StyleGentleHint,
    StyleSteady,
    StyleSteadyHint,
    StyleOff,
    StyleOffHint,
    QuietFrom,
    ReportWhen,
    GoalMet,
    Cards,
    ItemReview,
    ItemLesson,
    ItemWeakSpot,
    ItemReading,
    PhraseLesson,
    PhraseWeakSpot,
    PhraseReading,
    Snooze,
    SnoozedUntil,
    RecapTitle,
    StatTime,
    StatSteps,
    StatReviews,
    StatNewWords,
    StatDays,
    StatRetention,
    StatPelita,
    // --- coach: report, rewards, journal ---
    ReportTitle,
    ReportLast,
    ReportThisWeek,
    SoFar,
    LeagueRank,
    LeagueXp,
    Buy,
    Use,
    JournalEmpty,
    NavJournal,
    NavStats,
    JournalOpen,
    JournalFilterAll,
    JournalFilterLearning,
    JournalFilterWords,
    JournalFilterReflections,
    JournalNothingHere,
    JournalShowAll,
    JournalShowOlder,
    JournalReflectionAdd,
    JournalReflectionSave,
    JournalReflectionCancel,
    JournalReflectionEdit,
    JournalReflectionDelete,
    JournalReflectionDeleteSure,
    JournalTestPassed,
    EvReviews,
    EvLessonStep,
    EvLessonDone,
    EvPlanItem,
    // --- play ---
    NavPlay,
    PlayAgain,
    PlayOtherGames,
    PlayFinish,
    PlayRight,
    Check,
    ShowMeaning,
    HideMeaning,
    AudioMissing,
    PagiTitle,
    PagiBlurb,
    PagiListen,
    PagiPlayAll,
    PagiOneByOne,
    PagiAllTranslations,
    PagiDone,
    PagiReadToday,
    DeckTitle,
    DeckBlurb,
    DeckMarkKnown,
    DeckKnown,
    DetectiveTitle,
    DetectiveBlurb,
    DetectiveAllRules,
    DetectiveShowAllRules,
    DetectiveShowFewerRules,
    SoundVowel,
    SoundDropped,
    SoundSukun,
    SoundWordEnd,
    AlarmTitle,
    AlarmBlurb,
    AlarmIntro,
    AlarmLooksLike,
    RupiahTitle,
    RupiahBlurb,
    RupiahHundreds,
    RupiahThousands,
    RupiahMillions,
    RupiahMixed,
    RupiahBest,
    RupiahWrong,
    RupiahNewBest,
    PasarTitle,
    PasarBlurb,
    PasarAskPrice,
    PasarAskLower,
    PasarOffer,
    PasarAccept,
    PasarRefuse,
    PasarDealIn,
    PasarDealOver,
    PasarWalked,
    PasarPaid,
    ReaderPlaceholder,
    ReaderRead,
    ReaderScreenshot,
    ReaderDrop,
    ReaderDropText,
    ReaderRecent,
    ReaderNew,
    LangIndonesian,
    LangJavanese,
    LangEnglish,
    LangProper,
    LangUnknown,
    StatusNew,
    StatusLearning,
    StatusKnown,
    StatusIgnored,
    AddedToReview,
    Listen,
    Unverified,
    NoLocalGloss,
    MorphTimeWords,
    MorphGrammar,
    MorphColours,
    Normalized,
    BridgeTitle,
    ShiftSame,
    ShiftNarrowed,
    ShiftBroadened,
    ShiftShifted,
    FalseFriend,
    SpeechLevels,
    LevelNgoko,
    LevelMadya,
    LevelKrama,
    LevelKramaInggil,
    LevelKramaAndhap,
    TaughtByYou,
    KabarDictionary,
    DraftWaiting,
    TeachKabar,
    TeachKabarHint,
    InIndonesian,
    Helpful,
    NotHelpful,
    SavePhrase,
    PhraseSaved,
    RemoveHighlight,
    ExplainSentence,
    ExplainNeedsCloud,
    ExplainResetsLater,
    ExplainUnreachable,
    ExplainTryAgain,
    InOtherWords,
    RegisterChat,
    RegisterFormal,
    RegisterNeutral,
    RegisterSlang,
    RegisterKrama,
    RegisterNgoko,
    RegisterMixed,
    CultureNote,
    GrammarInSentence,
    GrammarInText,
    GrammarForWord,
    GrammarLearned,
    GrammarAtLevel,
    GrammarNotYet,
    UrlFetching,
    UrlImportFailed,
    OcrWorking,
    WaitWordPrompt,
    OcrFailed,
    OcrUnavailable,
    PickFailed,
    ScreenshotEmpty,
    ShareFirstImageOnly,
    SharedFrom,
    Close,
    /// The word sheet's drag handle (screen readers, keyboard).
    SheetResize,
    /// The word sheet's name for screen readers.
    WordSheet,
    Me,
    PenHint,
    // Reader: the translation under each line
    TranslationTitle,
    TranslationWorking,
    TranslationByAi,
    TranslationWordByWordShort,
    TranslationNeedsCloud,
    TranslationResetsLater,
    TranslationUnreachable,
    TranslationTryAgain,
    TranslationRetry,
    ReaderWriteFailed,
    ReadSmaller,
    ReadNormal,
    ReadLarger,
    ReadLargest,
    ReadSnug,
    ReadAiry,
    ReadWidth,
    ReadNarrow,
    ReadBook,
    ReadWide,
    ReadFocus,
    ReadFocusLeave,
    // Unggah-Ungguh Simulator (M11)
    UnggahTitle,
    UnggahBlurb,
    UnggahOpensAt,
    UnggahTurn,
    UnggahFits,
    UnggahTooPlain,
    UnggahTooStiff,
    UnggahSelfRaised,
    UnggahScenes,
    UnggahEndGreat,
    UnggahEndGood,
    UnggahEndGentle,
    // Minimal Pairs (M11)
    PairsTitle,
    PairsBlurb,
    PairsBoth,
    PairsListenMode,
    PairsReadMode,
    PairsNoJavaneseVoice,
    PairsHear,
    PairsWhichHeard,
    PairsWhichSpelling,
    PairsJavanese,
    PairsIndonesian,
    PronModeOn,
    PronModeTap,
    PronModeOff,
    PronTitle,
    PronShow,
    PronListen,
    PronHowToRead,
    PronGuideIntro,
    PronGuideNote,
    // Preference save recovery
    SaveFailed,
    SaveRetry,
    /// The web app: another tab took over the browser's copy of the data.
    WebOtherTab,
    /// The web app: the browser refused to keep the data.
    WebSaveFailed,
    ProblemWrongPassword,
    ProblemLocked,
    ProblemTaken,
    ProblemBadUsername,
    ProblemShortPassword,
    ProblemMismatch,
}
}

/// Fills a template from this catalog: `{n}` as written, `{n|one|two|few[|many]}`
/// with Arabic's dual and plural (`kabar_core::coach::voice::fill`).
pub fn fill(lang: UiLanguage, template: &str, args: &[(&str, &str)]) -> String {
    kabar_core::coach::voice::fill(lang, template, args)
}

/// A typed UI string from any catalog: the shared [`Msg`] or a feature's
/// own (`coach::msg::CoachMsg`, ...).
pub trait Text: Copy {
    fn text(self, lang: UiLanguage) -> &'static str;
}

impl Text for Msg {
    fn text(self, lang: UiLanguage) -> &'static str {
        msg_text(lang, self)
    }
}

/// A UI string in `lang`, from whichever catalog `msg` belongs to.
pub fn t<M: Text>(lang: UiLanguage, msg: M) -> &'static str {
    msg.text(lang)
}

fn msg_text(lang: UiLanguage, msg: Msg) -> &'static str {
    use Msg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (WelcomeKabarFrom, ArEg) => "كلمة \u{2068}kabar\u{2069} جاية من العربي",
        (WelcomeKabarFrom, Ar) => "كلمة \u{2068}kabar\u{2069} أصلها عربي",
        (WelcomeKabarFrom, En) => "kabar comes from Arabic",
        (WelcomeExplain, ArEg) => {
            "«\u{2068}Apa kabar?\u{2069}» يعني «إيه الأخبار؟» — وإنت أصلاً عارف الكلمة دي."
        }
        (WelcomeExplain, Ar) => "«\u{2068}Apa kabar?\u{2069}» تعني «ما الأخبار؟» — وأنت تعرف هذه الكلمة أصلًا.",
        (WelcomeExplain, En) => "“Apa kabar?” means “what's the news?”, and you already know that word.",
        (WelcomeFirstWord, ArEg) => "أول كلمة ليك: كلمة إنت عارفها.",
        (WelcomeFirstWord, Ar) => "كلمتك الأولى: كلمة تعرفها بالفعل.",
        (WelcomeFirstWord, En) => "Your first word is one you already know.",
        (WelcomeStart, ArEg) => "يلا نبدأ",
        (WelcomeStart, Ar) => "لنبدأ",
        (WelcomeStart, En) => "Let's begin",
        (NavToday, ArEg | Ar) => "اليوم",
        (NavToday, En) => "Today",
        (NavJourney, ArEg | Ar) => "الرحلة",
        (NavJourney, En) => "Journey",
        (NavReader, ArEg | Ar) => "القارئ",
        (NavReader, En) => "Reader",
        (NavReview, ArEg | Ar) => "المراجعة",
        (NavReview, En) => "Review",
        (NavCoach, _) => "Kak Nur",
        (NavStudio, ArEg | Ar) => "الاستوديو",
        (NavStudio, En) => "Studio",
        (NavSettings, ArEg | Ar) => "الإعدادات",
        (NavSettings, En) => "Settings",
        (NavGroupLearn, ArEg) => "اتعلّم",
        (NavGroupLearn, Ar) => "تعلّم",
        (NavGroupLearn, En) => "Learn",
        (NavGroupPractise, ArEg) => "اتمرّن",
        (NavGroupPractise, Ar) => "تدرّب",
        (NavGroupPractise, En) => "Practise",
        (NavGroupMe, ArEg | Ar) => "أنا",
        (NavGroupMe, En) => "Me",
        (NavPenLab, _) => "Pen Lab",
        (NavTeach, ArEg | Ar) => "علّم \u{2068}Kabar\u{2069}",
        (NavTeach, En) => "Teach Kabar",
        (NavListen, ArEg) => "اتفرج واسمع",
        (NavListen, Ar) => "شاهد واستمع",
        (NavListen, En) => "Watch & listen",
        (NavChat, ArEg) => "دردش",
        (NavChat, Ar) => "محادثة",
        (NavChat, En) => "Chat",
        (TodayTitle, _) => "Rencana Hari Ini",
        (ComingIn, ArEg) => "جاي في",
        (ComingIn, Ar) => "قادم في",
        (ComingIn, En) => "Coming in",
        (ThemeSystem, ArEg) => "زي الجهاز",
        (ThemeSystem, Ar) => "حسب النظام",
        (ThemeSystem, En) => "System",
        (ThemeSystemBlack, ArEg) => "زي الجهاز، وبالليل أسود",
        (ThemeSystemBlack, Ar) => "حسب النظام، أسود ليلًا",
        (ThemeSystemBlack, En) => "System, true black at night",
        (ThemeLight, ArEg | Ar) => "فاتح",
        (ThemeLight, En) => "Light",
        (ThemeDark, ArEg | Ar) => "داكن",
        (ThemeDark, En) => "Dark",
        (ThemeBlack, ArEg | Ar) => "أسود AMOLED",
        (ThemeBlack, En) => "True black (AMOLED)",
        (ThemeNight, ArEg | Ar) => "ليلي",
        (ThemeNight, En) => "Night",
        (ThemeBatik, ArEg | Ar) => "نيلي باتيك",
        (ThemeBatik, En) => "Batik indigo",
        (ThemeSawah, ArEg) => "أخضر غيطان",
        (ThemeSawah, Ar) => "أخضر الحقول",
        (ThemeSawah, En) => "Sawah green",
        (CostTitle, ArEg | Ar) => "التكلفة والحدود",
        (CostTitle, En) => "Cost & limits",
        (CostUnitRequests, ArEg | Ar) => "طلب",
        (CostUnitRequests, En) => "requests",
        (CostUnitCharacters, ArEg | Ar) => "حرف",
        (CostUnitCharacters, En) => "characters",
        (CostUnitNeurons, ArEg | Ar) => "\u{2068}Neurons\u{2069}",
        (CostUnitNeurons, En) => "Neurons",
        (CostUnitRowsRead, ArEg | Ar) => "صف مقروء",
        (CostUnitRowsRead, En) => "rows read",
        (CostUnitRowsWritten, ArEg | Ar) => "صف مكتوب",
        (CostUnitRowsWritten, En) => "rows written",
        (CostPerDay, ArEg | Ar) => "في اليوم",
        (CostPerDay, En) => "a day",
        (CostPerMonth, ArEg | Ar) => "في الشهر",
        (CostPerMonth, En) => "a month",
        (DiagTitle, ArEg) => "اللي حصل مؤخرًا",
        (DiagTitle, Ar) => "ما حدث مؤخرًا",
        (DiagTitle, En) => "What happened lately",
        (DiagEmpty, ArEg) => "مفيش حاجة غلط من ساعة ما فتحت التطبيق.",
        (DiagEmpty, Ar) => "لم يحدث أي خطأ منذ فتح التطبيق.",
        (DiagEmpty, En) => "Nothing has gone wrong since Kabar opened.",
        (DiagCount, ArEg) => "{n|سطر|سطرين|سطور}",
        (DiagCount, Ar) => "{n|سطر|سطران|أسطر|سطرًا}",
        (DiagCount, En) => "{n|line|lines}",
        (DiagRefresh, ArEg) => "حدّث",
        (DiagRefresh, Ar) => "تحديث",
        (DiagRefresh, En) => "Refresh",
        (DiagCopy, ArEg) => "انسخ",
        (DiagCopy, Ar) => "نسخ",
        (DiagCopy, En) => "Copy",
        (DiagCopied, ArEg) => "اتنسخ",
        (DiagCopied, Ar) => "نُسخ",
        (DiagCopied, En) => "Copied",
        (ExportTitle, ArEg | Ar) => "بياناتك",
        (ExportTitle, En) => "Your data",
        (ExportButton, ArEg) => "صدّر كل حاجة",
        (ExportButton, Ar) => "تصدير كل شيء",
        (ExportButton, En) => "Export everything",
        (ExportSaving, ArEg) => "بنحفظ…",
        (ExportSaving, Ar) => "جارٍ الحفظ…",
        (ExportSaving, En) => "Saving…",
        (ExportSaved, ArEg) => "اتحفظ {rows|صف|صفين|صفوف} في:",
        (ExportSaved, Ar) => "حُفظ {rows|صفّ|صفّان|صفوف|صفًّا} في:",
        (ExportSaved, En) => "Saved {rows|row|rows} to:",
        (ExportFailed, ArEg) => "معرفناش نحفظ الملف. مفيش حاجة اتغيّرت، تحب تجرّب تاني؟",
        (ExportFailed, Ar) => "تعذّر حفظ الملف. لم يتغير شيء، هل تحاول مرة أخرى؟",
        (ExportFailed, En) => "Couldn't save the file. Nothing was changed; want to try again?",
        (ExportAndroidHint, ArEg) => {
            "على الأندرويد الملف بيتحفظ في فولدر \u{2068}Kabar\u{2069} نفسه، وتقدر تنقله لكمبيوتر بكابل \u{2068}USB\u{2069}."
        }
        (ExportAndroidHint, Ar) => {
            "على أندرويد يُحفظ الملف في مجلد \u{2068}Kabar\u{2069} نفسه، ويمكنك نقله إلى حاسوب عبر كابل \u{2068}USB\u{2069}."
        }
        (ExportAndroidHint, En) => {
            "On Android the file lands in Kabar's own folder; copy it to a computer over USB."
        }
        (PenLabClear, ArEg) => "امسح",
        (PenLabClear, Ar) => "امسح",
        (PenLabClear, En) => "Clear",
        (PenLabSave, ArEg | Ar) => "احفظ التقرير",
        (PenLabSave, En) => "Save report",
        (PenLabCopy, ArEg | Ar) => "انسخ JSON",
        (PenLabCopy, En) => "Copy JSON",
        (PenLabCopied, ArEg) => "اتنسخ",
        (PenLabCopied, Ar) => "تم النسخ",
        (PenLabCopied, En) => "Copied",
        (PenLabSaved, ArEg) => "اتحفظ",
        (PenLabSaved, Ar) => "تم الحفظ",
        (PenLabSaved, En) => "Saved",
        (PenLabEvents, ArEg | Ar) => "الأحداث",
        (PenLabEvents, En) => "Events",
        (PenLabSavedRuns, ArEg | Ar) => "التقارير المحفوظة",
        (PenLabSavedRuns, En) => "Saved reports",
        (Width, ArEg | Ar) => "عرض الشاشة",
        (Width, En) => "Window width",
        (WidthCompact, ArEg | Ar) => "ضيّق",
        (WidthCompact, En) => "Compact",
        (WidthMedium, ArEg | Ar) => "متوسط",
        (WidthMedium, En) => "Medium",
        (WidthExpanded, ArEg | Ar) => "عريض",
        (WidthExpanded, En) => "Expanded",
        (WidthLarge, ArEg | Ar) => "واسع",
        (WidthLarge, En) => "Large",
        (WidthExtraLarge, ArEg) => "واسع جدًا",
        (WidthExtraLarge, Ar) => "واسع جدًّا",
        (WidthExtraLarge, En) => "Extra large",
        (NavDevices, ArEg | Ar) => "الحساب والأجهزة",
        (NavDevices, En) => "Account & devices",
        (CloudNotConfigured, ArEg) => {
            "النسخة دي لسه مش متوصلة بـ \u{2068}Kabar Cloud\u{2069}. أول ما السيرفر يشتغل هتقدر تسجّل دخول من هنا، وكل حاجة محفوظة على جهازك لحد ساعتها."
        }
        (CloudNotConfigured, Ar) => {
            "هذه النسخة غير مربوطة بـ \u{2068}Kabar Cloud\u{2069} بعد. بعد تشغيل الخادم ستتمكن من تسجيل الدخول هنا، وكل شيء محفوظ على جهازك حتى ذلك الحين."
        }
        (CloudNotConfigured, En) => {
            "This build isn't connected to Kabar Cloud yet. Once the server is up you can sign in here; until then everything stays safe on this device."
        }
        (CloudSignIn, ArEg) => "سجّل دخول بجوجل",
        (CloudSignIn, Ar) => "تسجيل الدخول بحساب Google",
        (CloudSignIn, En) => "Sign in with Google",
        (SyncLive, ArEg | Ar) => "متزامن",
        (SyncLive, En) => "In sync",
        (SyncConnecting, ArEg) => "بيتصل…",
        (SyncConnecting, Ar) => "جارٍ الاتصال…",
        (SyncConnecting, En) => "Connecting…",
        (SyncCatchingUp, ArEg) => "بيجيب آخر التغييرات…",
        (SyncCatchingUp, Ar) => "جارٍ جلب آخر التغييرات…",
        (SyncCatchingUp, En) => "Catching up…",
        (SyncOffline, ArEg) => "مفيش اتصال دلوقتي. تغييراتك محفوظة على الجهاز وهتتبعت لوحدها.",
        (SyncOffline, Ar) => "لا يوجد اتصال الآن. تغييراتك محفوظة على الجهاز وستُرسل تلقائيًا.",
        (SyncOffline, En) => {
            "Offline for now. Your changes are saved on this device and will sync on their own."
        }
        (SyncStopped, ArEg) => "المزامنة واقفة",
        (SyncStopped, Ar) => "المزامنة متوقفة",
        (SyncStopped, En) => "Sync is paused",
        (ProblemNotAllowed, ArEg) => "الحساب ده مش من الحسابات المسموح بيها. جرّب الجيميل بتاعك.",
        (ProblemNotAllowed, Ar) => "هذا الحساب ليس من الحسابات المسموح بها. جرّب حساب Gmail الخاص بك.",
        (ProblemNotAllowed, En) => "That Google account isn't on Kabar's allowlist. Try your own Gmail.",
        (ProblemSignInCancelled, ArEg) => "التسجيل اتلغى. جرّب تاني وقت ما تحب.",
        (ProblemSignInCancelled, Ar) => "أُلغي تسجيل الدخول. حاول مجددًا متى شئت.",
        (ProblemSignInCancelled, En) => "Sign-in was cancelled. Try again whenever you like.",
        (ProblemSignInFailed, ArEg) => "التسجيل ما نجحش المرة دي. جرّب تاني.",
        (ProblemSignInFailed, Ar) => "لم ينجح تسجيل الدخول هذه المرة. حاول مجددًا.",
        (ProblemSignInFailed, En) => "Sign-in didn't work this time. Please try again.",
        (ProblemSignedOutRemotely, ArEg) => "الجهاز ده اتسجّل خروجه من جهاز تاني. سجّل دخول تاني لو حابب.",
        (ProblemSignedOutRemotely, Ar) => "سُجّل خروج هذا الجهاز من جهاز آخر. سجّل الدخول مجددًا إن أردت.",
        (ProblemSignedOutRemotely, En) => {
            "This device was signed out from another device. Sign in again whenever you like."
        }
        (ProblemNetwork, ArEg) => "مش قادر أوصل لـ \u{2068}Kabar Cloud\u{2069} دلوقتي. هفضل أحاول لوحدي.",
        (ProblemNetwork, Ar) => "تعذّر الوصول إلى \u{2068}Kabar Cloud\u{2069} الآن. سأواصل المحاولة تلقائيًا.",
        (ProblemNetwork, En) => "Can't reach Kabar Cloud right now. I'll keep trying.",
        (ProblemSignInNetwork, ArEg) => {
            "مقدرتش أوصل لـ \u{2068}Kabar Cloud\u{2069}. اتأكد إن النت شغال وجرّب تاني."
        }
        (ProblemSignInNetwork, Ar) => {
            "تعذّر الوصول إلى \u{2068}Kabar Cloud\u{2069}. تأكد من الاتصال وحاول مجددًا."
        }
        (ProblemSignInNetwork, En) => "Couldn't reach Kabar Cloud. Check your connection and try again.",
        (ProblemServer, ArEg) => "\u{2068}Kabar Cloud\u{2069} فيه مشكلة دلوقتي. هحاول تاني بعد شوية.",
        (ProblemServer, Ar) => "يواجه \u{2068}Kabar Cloud\u{2069} مشكلة الآن. سأحاول مجددًا بعد قليل.",
        (ProblemServer, En) => "Kabar Cloud is having trouble. I'll try again in a bit.",
        (ProblemOutdated, ArEg) => "المزامنة محتاجة نسخة أحدث من \u{2068}Kabar\u{2069}. حدّث التطبيق.",
        (ProblemOutdated, Ar) => "تحتاج المزامنة إلى نسخة أحدث من \u{2068}Kabar\u{2069}. حدّث التطبيق.",
        (ProblemOutdated, En) => "Sync needs a newer Kabar. Please update the app.",
        (DevicesRefresh, ArEg) => "حدّث",
        (DevicesRefresh, Ar) => "تحديث",
        (DevicesRefresh, En) => "Refresh",
        (JourneyIndonesian, ArEg | Ar) => "الإندونيسية",
        (JourneyIndonesian, En) => "Indonesian",
        (JourneyJavanese, ArEg | Ar) => "الجاوية",
        (JourneyJavanese, En) => "Javanese",
        (JourneyRoute, ArEg) => "المسار",
        (JourneyRoute, Ar) => "المسار",
        (JourneyRoute, En) => "Route",
        (JourneyUpNext, ArEg) => "اللي جاي",
        (JourneyUpNext, Ar) => "التالي",
        (JourneyUpNext, En) => "Up next",
        (LessonLearn, ArEg | Ar) => "تعلُّم",
        (LessonLearn, En) => "Learn",
        (LessonPractice, ArEg | Ar) => "تمرين",
        (LessonPractice, En) => "Practice",
        (LessonReview, ArEg | Ar) => "مراجعة",
        (LessonReview, En) => "Review",

        // ---- Review (M6) ----
        (RevSeedTitle, ArEg) => "ابدأ بكلمات إنت أصلاً عارفها",
        (RevSeedTitle, Ar) => "ابدأ بكلمات تعرفها أصلًا",
        (RevSeedTitle, En) => "Start with words you already know",
        (RevDue, ArEg) => "مستنية مراجعة",
        (RevDue, Ar) => "مستحقة الآن",
        (RevDue, En) => "Due now",
        (RevNew, ArEg | Ar) => "جديدة",
        (RevNew, En) => "New",
        (RevReviewedToday, ArEg) => "راجعت النهارده",
        (RevReviewedToday, Ar) => "مراجعات اليوم",
        (RevReviewedToday, En) => "Reviewed today",
        (RevRetention, ArEg | Ar) => "نسبة التذكّر",
        (RevRetention, En) => "Retention",
        (RevForecastEmpty, ArEg) => "مفيش مراجعات جاية لسه. أول ما تتعلم كلمات جديدة هتظهر هنا.",
        (RevForecastEmpty, Ar) => "لا مراجعات قادمة بعد. ستظهر هنا حين تتعلم كلمات جديدة.",
        (RevForecastEmpty, En) => "No reviews coming up yet. They show up here as you learn new words.",
        (RevAllDone, ArEg) => "مفيش حاجة مستنياك دلوقتي. استريّح، إنت عامل شغل حلو.",
        (RevAllDone, Ar) => "لا شيء مستحق الآن. خذ راحتك، فأنت تُحسن العمل.",
        (RevAllDone, En) => "Nothing is waiting for you right now. Rest easy, you're doing good work.",
        (RevHowWell, ArEg) => "افتكرتها قد إيه؟",
        (RevHowWell, Ar) => "إلى أي حد تذكّرتها؟",
        (RevHowWell, En) => "How well did you remember it?",
        (RevContentUnchecked, ArEg) => "صحة المحتوى: لسه ما راجعتهاش",
        (RevContentUnchecked, Ar) => "صحة المحتوى: لم تُراجَع بعد",
        (RevContentUnchecked, En) => "Content: not checked yet",
        (RevContentChecked, ArEg) => "صحة المحتوى: راجعتها ✓",
        (RevContentChecked, Ar) => "صحة المحتوى: رُوجعت ✓",
        (RevContentChecked, En) => "Content: checked ✓",
        (RateAgain, ArEg) => "تاني",
        (RateAgain, Ar) => "مجددًا",
        (RateAgain, En) => "Again",
        (RateHard, ArEg) => "صعبة",
        (RateHard, Ar) => "صعب",
        (RateHard, En) => "Hard",
        (RateGood, ArEg) => "تمام",
        (RateGood, Ar) => "جيد",
        (RateGood, En) => "Good",
        (RateEasy, ArEg) => "سهلة",
        (RateEasy, Ar) => "سهل",
        (RateEasy, En) => "Easy",
        (RevPromptMeaning, ArEg) => "معناها إيه؟",
        (RevPromptMeaning, Ar) => "ما معناها؟",
        (RevPromptMeaning, En) => "What does it mean?",
        (RevPromptProduce, ArEg) => "بتتقال إزاي؟ اكتبها.",
        (RevPromptProduce, Ar) => "كيف تُقال؟ اكتبها.",
        (RevPromptProduce, En) => "How do you say it? Write it.",
        (RevPromptListen, ArEg) => "اسمع: معناها إيه؟",
        (RevPromptListen, Ar) => "استمع: ما معناها؟",
        (RevPromptListen, En) => "Listen: what does it mean?",
        (RevPromptCloze, ArEg) => "إيه الكلمة الناقصة؟",
        (RevPromptCloze, Ar) => "ما الكلمة الناقصة؟",
        (RevPromptCloze, En) => "Which word fills the gap?",
        (RevPromptBridge, ArEg) => "أنهي كلمة إندونيسي جت من دي؟",
        (RevPromptBridge, Ar) => "أي كلمة إندونيسية جاءت من هذه؟",
        (RevPromptBridge, En) => "Which Indonesian word came from this?",
        (RevFromArabic, ArEg | Ar) => "أصلها عربي",
        (RevFromArabic, En) => "From Arabic",
        (RevFalseFriend, ArEg) => "خلّي بالك: صاحب مخادع!",
        (RevFalseFriend, Ar) => "انتبه: صديق مخادع!",
        (RevFalseFriend, En) => "False friend!",
        (RevUnverified, ArEg) => "لسه متراجعتش",
        (RevUnverified, Ar) => "لم تُراجَع بعد",
        (RevUnverified, En) => "unverified",
        (RevTapToReveal, ArEg) => "دوس عشان تشوف المعنى",
        (RevTapToReveal, Ar) => "انقر لترى المعنى",
        (RevTapToReveal, En) => "Tap to see the meaning",
        (RevDoneTitle, ArEg) => "عاش! خلّصت الجلسة",
        (RevDoneTitle, Ar) => "أحسنت! انتهت الجلسة",
        (RevDoneTitle, En) => "Nicely done!",
        (RevCards, ArEg) => "كروت",
        (RevCards, Ar) => "بطاقات",
        (RevCards, En) => "Cards",
        (RevRemembered, ArEg) => "افتكرتها",
        (RevRemembered, Ar) => "تذكّرتها",
        (RevRemembered, En) => "Remembered",
        (RevTime, ArEg | Ar) => "الوقت",
        (RevTime, En) => "Time",
        (RevStrongest, ArEg) => "أقوى كلمة النهارده",
        (RevStrongest, Ar) => "أقوى كلمة اليوم",
        (RevStrongest, En) => "Strongest today",
        (RevRevisit, ArEg) => "دول هيرجعولك قريب، وكده بيثبتوا",
        (RevRevisit, Ar) => "ستعود إليك هذه قريبًا، وهكذا تثبت",
        (RevRevisit, En) => "Coming back soon; that's how words stick",
        (RevBackHome, ArEg) => "ارجع للمراجعة",
        (RevBackHome, Ar) => "العودة إلى المراجعة",
        (RevBackHome, En) => "Back to Review",
        (RevNothingHere, ArEg) => "مفيش كروت للطريقة دي دلوقتي.",
        (RevNothingHere, Ar) => "لا توجد بطاقات لهذه الطريقة الآن.",
        (RevNothingHere, En) => "No cards for this way of reviewing right now.",
        (RevSaveFailed, ArEg) => "معرفتش أحفظ الإجابة دي. جرّب تاني.",
        (RevSaveFailed, Ar) => "تعذّر حفظ هذه الإجابة. حاول مجددًا.",
        (RevSaveFailed, En) => "Couldn't save that answer. Please try again.",
        (RevUndoFailed, ArEg) => "معرفتش أرجّع الإجابة. لسه مسجّلة؛ جرّب تاني.",
        (RevUndoFailed, Ar) => "تعذّر التراجع عن الإجابة. لا تزال مسجّلة؛ حاول مجددًا.",
        (RevUndoFailed, En) => "Couldn't undo that answer. It is still saved; please try again.",

        // --- coach: general ---
        (Back, ArEg) => "رجوع",
        (Back, Ar) => "السابق",
        (Back, En) => "Back",
        (Next, ArEg) => "اللي بعده",
        (Next, Ar) => "التالي",
        (Next, En) => "Next",
        (Start, ArEg) => "يلا",
        (Start, Ar) => "ابدأ",
        (Start, En) => "Start",
        (Undo, ArEg) => "رجّعها",
        (Undo, Ar) => "تراجع",
        (Undo, En) => "Undo",
        (Dismiss, ArEg) => "تمام",
        (Dismiss, Ar) => "حسنًا",
        (Dismiss, En) => "OK",
        (ShowMore, ArEg) => "اعرض أكتر",
        (ShowMore, Ar) => "اعرض المزيد",
        (ShowMore, En) => "Show more",
        (TodayWord, ArEg | Ar) => "اليوم",
        (TodayWord, En) => "Today",
        (Yesterday, ArEg) => "إمبارح",
        (Yesterday, Ar) => "أمس",
        (Yesterday, En) => "Yesterday",
        (Mon, ArEg | Ar) => "الاثنين",
        (Mon, En) => "Monday",
        (Tue, ArEg | Ar) => "الثلاثاء",
        (Tue, En) => "Tuesday",
        (Wed, ArEg | Ar) => "الأربعاء",
        (Wed, En) => "Wednesday",
        (Thu, ArEg | Ar) => "الخميس",
        (Thu, En) => "Thursday",
        (Fri, ArEg | Ar) => "الجمعة",
        (Fri, En) => "Friday",
        (Sat, ArEg | Ar) => "السبت",
        (Sat, En) => "Saturday",
        (Sun, ArEg | Ar) => "الأحد",
        (Sun, En) => "Sunday",
        (SkillReading, ArEg | Ar) => "القراءة",
        (SkillReading, En) => "Reading",
        (SkillListening, ArEg | Ar) => "الاستماع",
        (SkillListening, En) => "Listening",
        (SkillWriting, ArEg | Ar) => "الكتابة",
        (SkillWriting, En) => "Writing",
        (SkillSpeaking, ArEg) => "الكلام",
        (SkillSpeaking, Ar) => "التحدث",
        (SkillSpeaking, En) => "Speaking",
        (SkillVocabulary, ArEg | Ar) => "المفردات",
        (SkillVocabulary, En) => "Vocabulary",

        (WhyTravel, ArEg | Ar) => "السفر",
        (WhyTravel, En) => "Travel",
        (WhyWork, ArEg | Ar) => "الشغل",
        (WhyWork, En) => "Work",
        (WhyFamily, ArEg) => "عيلة وصحاب",
        (WhyFamily, Ar) => "العائلة والأصدقاء",
        (WhyFamily, En) => "Family & friends",
        (WhyMedia, ArEg) => "أفلام وأغاني وسوشيال",
        (WhyMedia, Ar) => "الأفلام والأغاني ووسائل التواصل",
        (WhyMedia, En) => "Films, music & social media",
        (WhyFaith, ArEg | Ar) => "الإسلام في إندونيسيا وجاوة",
        (WhyFaith, En) => "Islam in Indonesia & Java",
        (WhyCuriosity, ArEg) => "فضول وحب للغات",
        (WhyCuriosity, Ar) => "الفضول وحب اللغات",
        (WhyCuriosity, En) => "Curiosity",
        (FocusBoth, ArEg) => "الاتنين",
        (FocusBoth, Ar) => "كلتاهما",
        (FocusBoth, En) => "Both",
        (OnbBothOrder, ArEg) => "هنبدأ بالإندونيسي، والجاوي بيفتح بعد \u{2068}BIPA 1\u{2069}.",
        (OnbBothOrder, Ar) => "نبدأ بالإندونيسية، وتُفتح الجاوية بعد \u{2068}BIPA 1\u{2069}.",
        (OnbBothOrder, En) => "We start with Indonesian; Javanese opens after BIPA 1.",
        (OnbNoTarget, ArEg) => "من غير ميعاد محدد",
        (OnbNoTarget, Ar) => "دون موعد محدد",
        (OnbNoTarget, En) => "No fixed date",
        (OnbNoTargetChosen, ArEg) => "تمام، هنمشي على راحتنا من غير ميعاد.",
        (OnbNoTargetChosen, Ar) => "حسنًا، سنمضي على مهل دون موعد محدد.",
        (OnbNoTargetChosen, En) => "Fine, we'll go at your pace with no deadline.",
        (OnbSetTarget, ArEg) => "حط هدف",
        (OnbSetTarget, Ar) => "حدّد هدفًا",
        (OnbSetTarget, En) => "Set a target",
        (StartNew, ArEg) => "لسه هبدأ من الصفر",
        (StartNew, Ar) => "أبدأ من الصفر",
        (StartNew, En) => "Brand new",
        (StartNewHint, ArEg) => "ولا يهمك، أول كلمة إنت أصلاً عارفها.",
        (StartNewHint, Ar) => "لا بأس، فأول كلمة تعرفها أصلًا.",
        (StartNewHint, En) => "No problem: your first word is one you already know.",
        (StartBasics, ArEg) => "عارف شوية أساسيات",
        (StartBasics, Ar) => "أعرف بعض الأساسيات",
        (StartBasics, En) => "I know some basics",
        (StartBasicsHint, ArEg) => "التحيات، الأرقام، جمل بسيطة.",
        (StartBasicsHint, Ar) => "التحيات والأرقام وجمل بسيطة.",
        (StartBasicsHint, En) => "Greetings, numbers, simple sentences.",
        (StartChat, ArEg) => "أقدر أدردش شوية",
        (StartChat, Ar) => "أستطيع إجراء محادثة بسيطة",
        (StartChat, En) => "I can hold a simple chat",
        (StartChatHint, ArEg) => "بتفهم وبترد في مواقف كل يوم.",
        (StartChatHint, Ar) => "تفهم وتردّ في مواقف الحياة اليومية.",
        (StartChatHint, En) => "You understand and reply in everyday situations.",
        (OnbFinish, ArEg) => "يلا بينا",
        (OnbFinish, Ar) => "لننطلق",
        (OnbFinish, En) => "Let's go",
        (ErrNoStudyTime, ArEg) => "اختار ميعاد واحد على الأقل للمذاكرة.",
        (ErrNoStudyTime, Ar) => "اختر موعدًا واحدًا على الأقل للدراسة.",
        (ErrNoStudyTime, En) => "Pick at least one study time.",
        (ErrTargetInPast, ArEg) => "تاريخ الهدف لازم يكون بعد النهارده.",
        (ErrTargetInPast, Ar) => "يجب أن يكون تاريخ الهدف بعد اليوم.",
        (ErrTargetInPast, En) => "The target date needs to be after today.",
        (ErrUnknownLevel, ArEg) => "المستوى ده مش موجود، اختار واحد من القايمة.",
        (ErrUnknownLevel, Ar) => "هذا المستوى غير موجود، اختر واحدًا من القائمة.",
        (ErrUnknownLevel, En) => "That level doesn't exist; pick one from the list.",

        (AddTime, ArEg) => "ميعاد كمان",
        (AddTime, Ar) => "موعد آخر",
        (AddTime, En) => "Add a time",
        (RemoveTime, ArEg | Ar) => "احذف",
        (RemoveTime, En) => "Remove",
        (StyleGentle, ArEg) => "هادي",
        (StyleGentle, Ar) => "لطيف",
        (StyleGentle, En) => "Gentle",
        (StyleGentleHint, ArEg) => "تذكير واحد بالكتير في اليوم.",
        (StyleGentleHint, Ar) => "تذكير واحد على الأكثر يوميًا.",
        (StyleGentleHint, En) => "At most one a day.",
        (StyleSteady, ArEg) => "منتظم",
        (StyleSteady, Ar) => "منتظم",
        (StyleSteady, En) => "Steady",
        (StyleSteadyHint, ArEg) => "في كل ميعاد لحد ما تخلّص هدف اليوم.",
        (StyleSteadyHint, Ar) => "عند كل موعد حتى تُنجز هدف اليوم.",
        (StyleSteadyHint, En) => "At each study time until today's goal is met.",
        (StyleOff, ArEg) => "من غير تذكيرات",
        (StyleOff, Ar) => "بلا تذكيرات",
        (StyleOff, En) => "Off",
        (StyleOffHint, ArEg) => "التقرير الأسبوعي هيفضل يوصلك جوه التطبيق.",
        (StyleOffHint, Ar) => "سيبقى التقرير الأسبوعي متاحًا داخل التطبيق.",
        (StyleOffHint, En) => "The weekly report still arrives in the app.",
        (QuietFrom, ArEg | Ar) => "من",
        (QuietFrom, En) => "From",
        (ReportWhen, ArEg) => "ميعاد التقرير الأسبوعي",
        (ReportWhen, Ar) => "موعد التقرير الأسبوعي",
        (ReportWhen, En) => "Weekly report",

        (GoalMet, ArEg) => "هدف النهارده خلص",
        (GoalMet, Ar) => "تحقق هدف اليوم",
        (GoalMet, En) => "Today's goal met",
        (Cards, ArEg | Ar) => "بطاقة",
        (Cards, En) => "cards",
        (ItemReview, ArEg | Ar) => "مراجعة",
        (ItemReview, En) => "Review",
        (ItemLesson, ArEg | Ar) => "درس",
        (ItemLesson, En) => "Lesson",
        (ItemWeakSpot, ArEg) => "تمرين على نقطة محتاجة شغل",
        (ItemWeakSpot, Ar) => "تمرين على نقطة تحتاج إلى عمل",
        (ItemWeakSpot, En) => "Practice a tricky spot",
        (ItemReading, ArEg) => "قراية أو سماع قصير",
        (ItemReading, Ar) => "قراءة أو استماع قصير",
        (ItemReading, En) => "A short read or listen",
        (PhraseLesson, ArEg | Ar) => "درس {t}",
        (PhraseLesson, En) => "the {t} lesson",
        (PhraseWeakSpot, ArEg) => "تمرين على {t}",
        (PhraseWeakSpot, Ar) => "تمرين على {t}",
        (PhraseWeakSpot, En) => "some practice on {t}",
        (PhraseReading, ArEg) => "قراية أو سماع قصير",
        (PhraseReading, Ar) => "قراءة أو استماع قصير",
        (PhraseReading, En) => "a short read or listen",
        (Snooze, ArEg) => "أجّل التذكيرات ساعة",
        (Snooze, Ar) => "أجّل التذكيرات ساعة",
        (Snooze, En) => "Snooze reminders for an hour",
        (SnoozedUntil, ArEg) => "التذكيرات مستريحة لحد",
        (SnoozedUntil, Ar) => "التذكيرات مؤجلة حتى",
        (SnoozedUntil, En) => "Reminders are resting until",
        (RecapTitle, ArEg) => "خلاصة النهارده",
        (RecapTitle, Ar) => "خلاصة اليوم",
        (RecapTitle, En) => "Today in 30 seconds",
        (StatTime, ArEg | Ar) => "الوقت",
        (StatTime, En) => "Time",
        (StatSteps, ArEg | Ar) => "خطوات",
        (StatSteps, En) => "Steps",
        (StatReviews, ArEg | Ar) => "مراجعات",
        (StatReviews, En) => "Reviews",
        (StatNewWords, ArEg | Ar) => "كلمات جديدة",
        (StatNewWords, En) => "New words",
        (StatDays, ArEg) => "أيام مذاكرة",
        (StatDays, Ar) => "أيام دراسة",
        (StatDays, En) => "Days studied",
        (StatRetention, ArEg) => "اللي فاكره",
        (StatRetention, Ar) => "نسبة التذكّر",
        (StatRetention, En) => "Retention",
        (StatPelita, _) => "Pelita",

        // --- coach: report, rewards, journal ---
        (ReportTitle, ArEg | Ar) => "التقرير الأسبوعي",
        (ReportTitle, En) => "Weekly report",
        (ReportLast, ArEg) => "آخر تقرير",
        (ReportLast, Ar) => "أحدث تقرير",
        (ReportLast, En) => "Latest report",
        (ReportThisWeek, ArEg) => "الأسبوع ده",
        (ReportThisWeek, Ar) => "هذا الأسبوع",
        (ReportThisWeek, En) => "This week",
        (SoFar, ArEg) => "لحد دلوقتي",
        (SoFar, Ar) => "حتى الآن",
        (SoFar, En) => "so far",
        (LeagueRank, ArEg) => "ترتيب الأسبوع ده",
        (LeagueRank, Ar) => "ترتيب هذا الأسبوع",
        (LeagueRank, En) => "This week's rank",
        (LeagueXp, ArEg | Ar) => "نقاط الخبرة",
        (LeagueXp, En) => "XP",
        (Buy, ArEg) => "اشتري",
        (Buy, Ar) => "اشترِ",
        (Buy, En) => "Buy",
        (Use, ArEg) => "استخدمه",
        (Use, Ar) => "استخدم",
        (Use, En) => "Use",
        (JournalEmpty, ArEg) => "كل اللي هتعمله هيتسجل هنا لوحده.",
        (JournalEmpty, Ar) => "سيُسجَّل هنا كل ما تنجزه تلقائيًا.",
        (JournalEmpty, En) => "Everything you do will be recorded here automatically.",
        (NavJournal, ArEg | Ar) => "اليوميات",
        (NavJournal, En) => "Journal",
        (NavStats, ArEg | Ar) => "التقدّم",
        (NavStats, En) => "Progress",
        (JournalOpen, ArEg) => "افتح اليوميات",
        (JournalOpen, Ar) => "افتح اليوميات",
        (JournalOpen, En) => "Open the journal",
        (JournalFilterAll, ArEg) => "الكل",
        (JournalFilterAll, Ar) => "الكل",
        (JournalFilterAll, En) => "All",
        (JournalFilterLearning, ArEg) => "مذاكرة",
        (JournalFilterLearning, Ar) => "دراسة",
        (JournalFilterLearning, En) => "Study",
        (JournalFilterWords, ArEg | Ar) => "كلمات",
        (JournalFilterWords, En) => "Words",
        (JournalFilterReflections, ArEg | Ar) => "خواطر",
        (JournalFilterReflections, En) => "Reflections",
        (JournalNothingHere, ArEg) => "لسه مفيش حاجة هنا. كل خطوة صغيرة هتظهر أول بأول.",
        (JournalNothingHere, Ar) => "لا شيء هنا بعد. ستظهر كل خطوة صغيرة أولًا بأول.",
        (JournalNothingHere, En) => "Nothing here yet. Every small step will show up as it happens.",
        (JournalShowAll, ArEg) => "اعرض كل اللي اتسجل",
        (JournalShowAll, Ar) => "اعرض كل ما سُجّل",
        (JournalShowAll, En) => "Show everything",
        (JournalShowOlder, ArEg) => "اعرض الأيام اللي قبل كده",
        (JournalShowOlder, Ar) => "اعرض الأيام الأقدم",
        (JournalShowOlder, En) => "Show older days",
        (JournalReflectionAdd, ArEg) => "اكتب خاطرة عن اليوم ده",
        (JournalReflectionAdd, Ar) => "اكتب خاطرة عن هذا اليوم",
        (JournalReflectionAdd, En) => "Write a reflection",
        (JournalReflectionSave, ArEg | Ar) => "احفظ",
        (JournalReflectionSave, En) => "Save",
        (JournalReflectionCancel, ArEg | Ar) => "إلغاء",
        (JournalReflectionCancel, En) => "Cancel",
        (JournalReflectionEdit, ArEg) => "عدّل",
        (JournalReflectionEdit, Ar) => "تعديل",
        (JournalReflectionEdit, En) => "Edit",
        (JournalReflectionDelete, ArEg) => "امسح",
        (JournalReflectionDelete, Ar) => "حذف",
        (JournalReflectionDelete, En) => "Delete",
        (JournalReflectionDeleteSure, ArEg) => "أيوه، امسحها",
        (JournalReflectionDeleteSure, Ar) => "نعم، احذفها",
        (JournalReflectionDeleteSure, En) => "Yes, delete it",
        (JournalTestPassed, ArEg) => "عدّيت اختبار المستوى",
        (JournalTestPassed, Ar) => "اجتزت اختبار المستوى",
        (JournalTestPassed, En) => "Passed the level test",
        (EvReviews, ArEg | Ar) => "مراجعة",
        (EvReviews, En) => "Reviews",
        (EvLessonStep, ArEg) => "خطوة في درس",
        (EvLessonStep, Ar) => "خطوة في درس",
        (EvLessonStep, En) => "Lesson step",
        (EvLessonDone, ArEg) => "خلّصت درس",
        (EvLessonDone, Ar) => "أنهيت درسًا",
        (EvLessonDone, En) => "Finished a lesson",
        (EvPlanItem, ArEg) => "خلّصت خطوة من خطة النهارده",
        (EvPlanItem, Ar) => "أنجزت خطوة من خطة اليوم",
        (EvPlanItem, En) => "Finished a step of the day's plan",
        // --- play ---
        (NavPlay, ArEg | Ar) => "ألعاب",
        (NavPlay, En) => "Play",
        (PlayAgain, ArEg) => "نلعب تاني",
        (PlayAgain, Ar) => "العب مجددًا",
        (PlayAgain, En) => "Play again",
        (PlayOtherGames, ArEg) => "لعبة تانية",
        (PlayOtherGames, Ar) => "لعبة أخرى",
        (PlayOtherGames, En) => "Another game",
        (PlayFinish, ArEg) => "خلّصنا",
        (PlayFinish, Ar) => "إنهاء",
        (PlayFinish, En) => "Finish",
        (PlayRight, ArEg) => "صح كده!",
        (PlayRight, Ar) => "أحسنت!",
        (PlayRight, En) => "That's it!",
        (Check, ArEg) => "اتأكد",
        (Check, Ar) => "تحقّق",
        (Check, En) => "Check",
        (ShowMeaning, ArEg) => "يعني إيه؟",
        (ShowMeaning, Ar) => "أظهر المعنى",
        (ShowMeaning, En) => "Show meaning",
        (HideMeaning, ArEg) => "خبّي المعنى",
        (HideMeaning, Ar) => "أخفِ المعنى",
        (HideMeaning, En) => "Hide meaning",
        (AudioMissing, ArEg) => "الصوت محتاج Kabar Cloud أو صوت إندونيسي على الجهاز. النص شغال عادي.",
        (AudioMissing, Ar) => "يحتاج الصوت إلى Kabar Cloud أو صوت إندونيسي على الجهاز. النص متاح كالمعتاد.",
        (AudioMissing, En) => "Audio needs Kabar Cloud or a device voice; the text works as usual.",
        // --- play: Kabar Pagi ---
        (PagiTitle, _) => "Kabar Pagi",
        (PagiBlurb, ArEg) => "حكاية صباحية في دقيقتين، من كلماتك وكام كلمة جديدة.",
        (PagiBlurb, Ar) => "قصة صباحية في دقيقتين، من كلماتك وبضع كلمات جديدة.",
        (PagiBlurb, En) => "A two-minute morning story from your words, plus a few new ones.",
        (PagiListen, ArEg) => "يلا نسمع",
        (PagiListen, Ar) => "لنستمع",
        (PagiListen, En) => "Let's listen",
        (PagiPlayAll, ArEg) => "شغّل الحكاية كلها",
        (PagiPlayAll, Ar) => "شغّل القصة كاملة",
        (PagiPlayAll, En) => "Play the whole story",
        (PagiOneByOne, ArEg) => "جملة جملة",
        (PagiOneByOne, Ar) => "جملة بعد جملة",
        (PagiOneByOne, En) => "One at a time",
        (PagiAllTranslations, ArEg) => "كل الترجمات",
        (PagiAllTranslations, Ar) => "كل الترجمات",
        (PagiAllTranslations, En) => "All translations",
        (PagiDone, ArEg) => "سمعتها وفهمتها",
        (PagiDone, Ar) => "استمعت إليها وفهمتها",
        (PagiDone, En) => "I've listened to it",
        (PagiReadToday, ArEg) => "حكاية النهارده خلصت. بكرة فيه واحدة جديدة.",
        (PagiReadToday, Ar) => "انتهت قصة اليوم. غدًا قصة جديدة.",
        (PagiReadToday, En) => "Today's story is done. A new one comes tomorrow.",
        // --- play: the Bridge deck ---
        (DeckTitle, ArEg) => "كلمات إنت عارفها أصلاً",
        (DeckTitle, Ar) => "كلمات تعرفها مسبقًا",
        (DeckTitle, En) => "Words you already know",
        (DeckBlurb, ArEg) => "كلمات جاية من العربي ومعناها زي ما إنت فاكره.",
        (DeckBlurb, Ar) => "كلمات أصلها عربي وبقي معناها كما تعرفه.",
        (DeckBlurb, En) => "Words from Arabic that kept the meaning you know.",
        (DeckMarkKnown, ArEg) => "عارفها",
        (DeckMarkKnown, Ar) => "أعرفها",
        (DeckMarkKnown, En) => "I know it",
        (DeckKnown, ArEg) => "بقت من كلماتك",
        (DeckKnown, Ar) => "صارت من كلماتك",
        (DeckKnown, En) => "in your words",

        // --- play: Sound-Shift Detective ---
        (DetectiveTitle, ArEg) => "مخبر الأصوات",
        (DetectiveTitle, Ar) => "محقق تحوّل الأصوات",
        (DetectiveTitle, En) => "Sound-Shift Detective",
        (DetectiveBlurb, ArEg) => "اعرف إزاي الأصوات العربي اتغيرت، وخمّن الأصل.",
        (DetectiveBlurb, Ar) => "تعرّف كيف تغيّرت الأصوات العربية، وخمّن الأصل.",
        (DetectiveBlurb, En) => "Learn how Arabic sounds changed, then guess the origin.",
        (DetectiveAllRules, ArEg) => "كل القواعد",
        (DetectiveAllRules, Ar) => "كل القواعد",
        (DetectiveAllRules, En) => "All rules",
        (DetectiveShowAllRules, ArEg) => "وريني كل القواعد ({n})",
        (DetectiveShowAllRules, Ar) => "اعرض كل القواعد ({n})",
        (DetectiveShowAllRules, En) => "Show every rule ({n})",
        (DetectiveShowFewerRules, ArEg) => "وريني الأشهر بس",
        (DetectiveShowFewerRules, Ar) => "اعرض الأشهر فقط",
        (DetectiveShowFewerRules, En) => "Just the common ones",
        // Placeholders on either side of a sound rule (content/bridge/sound-rules.json).
        (SoundVowel, ArEg | Ar) => "(حركة)",
        (SoundVowel, En) => "(vowel)",
        (SoundDropped, ArEg) => "(بيختفي)",
        (SoundDropped, Ar) => "(يُحذف)",
        (SoundDropped, En) => "(dropped)",
        (SoundSukun, ArEg | Ar) => "(سكون)",
        (SoundSukun, En) => "(no vowel)",
        (SoundWordEnd, ArEg | Ar) => "(آخر الكلمة)",
        (SoundWordEnd, En) => "(end of word)",
        // --- play: False Friends Alarm ---
        (AlarmTitle, ArEg) => "جرس الكلمات الخدّاعة",
        (AlarmTitle, Ar) => "تنبيه الأصدقاء المخادعين",
        (AlarmTitle, En) => "False Friends Alarm",
        (AlarmBlurb, ArEg) => "كلمات شكلها عربي بس معناها راح في حتة تانية.",
        (AlarmBlurb, Ar) => "كلمات تبدو عربية لكن معناها انتقل إلى شيء آخر.",
        (AlarmBlurb, En) => "Words that look Arabic but moved to another meaning.",
        (AlarmIntro, ArEg) => "بعض الكلمات بتضحك عليك: شكلها عربي ومعناها حاجة تانية خالص. يلا نمسكها.",
        (AlarmIntro, Ar) => "بعض الكلمات تخدعك: تبدو عربية ومعناها مختلف تمامًا. لنكشفها.",
        (AlarmIntro, En) => {
            "Some words play tricks: they look Arabic but mean something else. Let's catch them."
        }
        (AlarmLooksLike, ArEg) => "شبه",
        (AlarmLooksLike, Ar) => "تشبه",
        (AlarmLooksLike, En) => "Looks like",
        // --- play: Rupiah Reader ---
        (RupiahTitle, ArEg) => "اقرا الروبية",
        (RupiahTitle, Ar) => "قراءة الروبية",
        (RupiahTitle, En) => "Rupiah Reader",
        (RupiahBlurb, ArEg) => "اقرا الأسعار زي ما بتتكتب في المحلات، وبسرعة.",
        (RupiahBlurb, Ar) => "اقرأ الأسعار كما تُكتب في المتاجر، وبسرعة.",
        (RupiahBlurb, En) => "Read prices as shops write them, fast.",
        (RupiahHundreds, ArEg | Ar) => "مئات",
        (RupiahHundreds, En) => "Hundreds",
        (RupiahThousands, ArEg | Ar) => "آلاف",
        (RupiahThousands, En) => "Thousands",
        (RupiahMillions, ArEg | Ar) => "ملايين",
        (RupiahMillions, En) => "Millions",
        (RupiahMixed, ArEg) => "مكس",
        (RupiahMixed, Ar) => "منوّع",
        (RupiahMixed, En) => "Mixed",
        (RupiahBest, ArEg) => "أحسن نتيجة",
        (RupiahBest, Ar) => "أفضل نتيجة",
        (RupiahBest, En) => "best",
        (RupiahWrong, ArEg) => "رقم تاني، بس ولا يهمك. بيتقري كده:",
        (RupiahWrong, Ar) => "هذا رقم آخر، لا بأس. يُقرأ هكذا:",
        (RupiahWrong, En) => "A different number, no worries. It reads:",
        (RupiahNewBest, ArEg) => "أحسن نتيجة ليك في المستوى ده!",
        (RupiahNewBest, Ar) => "أفضل نتيجة لك في هذا المستوى!",
        (RupiahNewBest, En) => "Your best score at this level!",

        // --- play: Pasar! ---
        (PasarTitle, _) => "Pasar!",
        (PasarBlurb, ArEg) => "فاصل مع بياع في السوق وإنت معاك ميزانية.",
        (PasarBlurb, Ar) => "ساوم بائعًا في السوق ضمن ميزانيتك.",
        (PasarBlurb, En) => "Haggle with a market seller on a budget.",
        (PasarAskPrice, ArEg) => "اسأل عن السعر",
        (PasarAskPrice, Ar) => "اسأل عن السعر",
        (PasarAskPrice, En) => "Ask the price",
        (PasarAskLower, ArEg) => "اطلب تخفيض",
        (PasarAskLower, Ar) => "اطلب سعرًا أقل",
        (PasarAskLower, En) => "Ask for less",
        (PasarOffer, ArEg) => "اعرض سعر",
        (PasarOffer, Ar) => "اعرض سعرًا",
        (PasarOffer, En) => "Make an offer",
        (PasarAccept, ArEg) => "اتفقنا",
        (PasarAccept, Ar) => "موافق",
        (PasarAccept, En) => "Deal",
        (PasarRefuse, ArEg) => "اعتذر بذوق",
        (PasarRefuse, Ar) => "اعتذر بلطف",
        (PasarRefuse, En) => "Politely decline",
        (PasarDealIn, ArEg) => "صفقة حلوة وفي حدود ميزانيتك!",
        (PasarDealIn, Ar) => "صفقة جيدة ضمن ميزانيتك!",
        (PasarDealIn, En) => "A good deal, within budget!",
        (PasarDealOver, ArEg) => "اشتريتها، بس فوق الميزانية شوية. المرة الجاية فاصل أكتر.",
        (PasarDealOver, Ar) => "اشتريتها، لكن فوق الميزانية قليلًا. ساوم أكثر في المرة القادمة.",
        (PasarDealOver, En) => "Bought it, a little over budget. Haggle a bit more next time.",
        (PasarWalked, ArEg) => "مشيت بذوق، ودي كمان مهارة.",
        (PasarWalked, Ar) => "انصرفت بلطف، وهذه مهارة أيضًا.",
        (PasarWalked, En) => "You walked away politely; that's a skill too.",
        (PasarPaid, ArEg) => "دفعت:",
        (PasarPaid, Ar) => "دفعت:",
        (PasarPaid, En) => "You paid:",
        (TranslationTitle, ArEg) => "الترجمة",
        (TranslationTitle, Ar) => "الترجمة",
        (TranslationTitle, En) => "Translation",
        (TranslationWorking, ArEg) => "بترجملك الكلام كله…",
        (TranslationWorking, Ar) => "جارٍ ترجمة النص كله…",
        (TranslationWorking, En) => "Translating the whole text…",
        (TranslationByAi, ArEg) => "ترجمة بالذكاء الاصطناعي حسب السياق. دوس على أي كلمة لمعناها لوحدها.",
        (TranslationByAi, Ar) => "ترجمة بالذكاء الاصطناعي بحسب السياق. انقر أي كلمة لمعناها منفردة.",
        (TranslationByAi, En) => "AI translation in context. Tap any word for its own meaning.",
        (TranslationWordByWordShort, ArEg) => "كلمة بكلمة:",
        (TranslationWordByWordShort, Ar) => "كلمة بكلمة:",
        (TranslationWordByWordShort, En) => "Word by word:",
        (TranslationNeedsCloud, ArEg) => {
            "الترجمة الكاملة بتشتغل لما توصّل \u{2068}Kabar Cloud\u{2069}. لحد ده، تحت كل سطر معنى كلماته واحدة واحدة."
        }
        (TranslationNeedsCloud, Ar) => {
            "تعمل الترجمة الكاملة عند ربط \u{2068}Kabar Cloud\u{2069}. إلى ذلك الحين، تحت كل سطر معاني كلماته واحدة واحدة."
        }
        (TranslationNeedsCloud, En) => {
            "Full translation works once Kabar Cloud is connected. Until then, each line shows its words' meanings one by one."
        }
        (TranslationResetsLater, ArEg) => {
            "رصيد الترجمة المجاني خلص النهارده وبيرجع بكرة. تحت كل سطر معنى كلماته واحدة واحدة."
        }
        (TranslationResetsLater, Ar) => {
            "انتهى الرصيد المجاني للترجمة اليوم ويعود غدًا. تحت كل سطر معاني كلماته واحدة واحدة."
        }
        (TranslationResetsLater, En) => {
            "Today's free translation allowance is used up; it's back tomorrow. Each line shows its words' meanings one by one."
        }
        (TranslationUnreachable, ArEg) => {
            "مش قادر أوصل للترجمة دلوقتي (النت أو \u{2068}Kabar Home\u{2069}). معاني الكلمات اللي تحت شغالة من غير نت."
        }
        (TranslationUnreachable, Ar) => {
            "تعذّر الوصول إلى الترجمة الآن (الإنترنت أو \u{2068}Kabar Home\u{2069}). معاني الكلمات أدناه تعمل دون اتصال."
        }
        (TranslationUnreachable, En) => {
            "Can't reach the translation right now (offline, or Kabar Home is asleep). The word meanings below work offline."
        }
        (TranslationTryAgain, ArEg) => "الترجمة مطلعتش مظبوطة المرة دي. معاني الكلمات اللي تحت سليمة.",
        (TranslationTryAgain, Ar) => "لم تخرج الترجمة سليمة هذه المرة. معاني الكلمات أدناه سليمة.",
        (TranslationTryAgain, En) => {
            "The translation didn't come out right this time. The word meanings below are solid."
        }
        (TranslationRetry, ArEg) => "جرّب تاني",
        (TranslationRetry, Ar) => "أعد المحاولة",
        (TranslationRetry, En) => "Try again",
        (ReaderWriteFailed, ArEg) => "معرفتش أحفظ ده دلوقتي، ومفيش حاجة اتغيّرت. جرّب تاني بعد شوية.",
        (ReaderWriteFailed, Ar) => "تعذّر الحفظ الآن، ولم يتغيّر شيء. حاول مجددًا بعد قليل.",
        (ReaderWriteFailed, En) => {
            "Couldn't save that just now, so nothing changed. Please try again in a moment."
        }
        (ReadSmaller, ArEg | Ar) => "أصغر",
        (ReadSmaller, En) => "Smaller",
        (ReadNormal, ArEg | Ar) => "عادي",
        (ReadNormal, En) => "Normal",
        (ReadLarger, ArEg | Ar) => "أكبر",
        (ReadLarger, En) => "Larger",
        (ReadLargest, ArEg) => "أكبر كمان",
        (ReadLargest, Ar) => "الأكبر",
        (ReadLargest, En) => "Largest",
        (ReadSnug, ArEg) => "قريبة",
        (ReadSnug, Ar) => "متقاربة",
        (ReadSnug, En) => "Snug",
        (ReadAiry, ArEg) => "واسعة",
        (ReadAiry, Ar) => "متباعدة",
        (ReadAiry, En) => "Airy",
        (ReadWidth, ArEg | Ar) => "عرض السطر",
        (ReadWidth, En) => "Line width",
        (ReadNarrow, ArEg | Ar) => "ضيّق، والشرح تحت السطر",
        (ReadNarrow, En) => "Narrow, meanings under each line",
        (ReadBook, ArEg | Ar) => "زي الكتاب",
        (ReadBook, En) => "Book",
        (ReadWide, ArEg) => "على عرض الشاشة",
        (ReadWide, Ar) => "بعرض الشاشة",
        (ReadWide, En) => "Full width",
        (ReadFocus, ArEg | Ar) => "وضع التركيز",
        (ReadFocus, En) => "Focus mode",
        (ReadFocusLeave, ArEg) => "اخرج من التركيز",
        (ReadFocusLeave, Ar) => "الخروج من وضع التركيز",
        (ReadFocusLeave, En) => "Leave focus mode",
        (ReaderPlaceholder, ArEg) => "مثلاً: \u{2068}Saya sedang membaca buku.\u{2069}",
        (ReaderPlaceholder, Ar) => "مثال: \u{2068}Saya sedang membaca buku.\u{2069}",
        (ReaderPlaceholder, En) => "e.g. Saya sedang membaca buku.",
        (ReaderRead, ArEg) => "اقرا",
        (ReaderRead, Ar) => "اقرأ",
        (ReaderRead, En) => "Read",
        (ReaderScreenshot, ArEg) => "افتح سكرين شوت",
        (ReaderScreenshot, Ar) => "افتح لقطة شاشة",
        (ReaderScreenshot, En) => "Open a screenshot",
        (ReaderDrop, ArEg) => "أو اسحب صورة أو كلام وسيبه هنا",
        (ReaderDrop, Ar) => "أو اسحب صورة أو نصًا وأفلته هنا",
        (ReaderDrop, En) => "or drag an image or text here",
        (ReaderDropText, ArEg) => "أو اسحب كلام وسيبه هنا",
        (ReaderDropText, Ar) => "أو اسحب نصًا وأفلته هنا",
        (ReaderDropText, En) => "or drag text here",
        (ReaderRecent, ArEg) => "اللي قريته قريب",
        (ReaderRecent, Ar) => "قرأته مؤخرًا",
        (ReaderRecent, En) => "Recently read",
        (ReaderNew, ArEg | Ar) => "نص جديد",
        (ReaderNew, En) => "New text",
        (LangIndonesian, ArEg) => "إندونيسي",
        (LangIndonesian, Ar) => "إندونيسية",
        (LangIndonesian, En) => "Indonesian",
        (LangJavanese, ArEg) => "جاوي",
        (LangJavanese, Ar) => "جاوية",
        (LangJavanese, En) => "Javanese",
        (LangEnglish, ArEg) => "إنجليزي",
        (LangEnglish, Ar) => "إنجليزية",
        (LangEnglish, En) => "English loan",
        (LangProper, ArEg | Ar) => "اسم علم",
        (LangProper, En) => "Name",
        (LangUnknown, ArEg) => "لسه مش متأكدين",
        (LangUnknown, Ar) => "غير محددة بعد",
        (LangUnknown, En) => "Not sure yet",
        (StatusNew, ArEg | Ar) => "جديدة",
        (StatusNew, En) => "New",
        (StatusLearning, ArEg) => "بتتعلمها",
        (StatusLearning, Ar) => "قيد التعلم",
        (StatusLearning, En) => "Learning",
        (StatusKnown, ArEg) => "عارفها",
        (StatusKnown, Ar) => "أعرفها",
        (StatusKnown, En) => "Known",
        (StatusIgnored, ArEg | Ar) => "تجاهل",
        (StatusIgnored, En) => "Ignore",
        (AddedToReview, ArEg) => "اتضافت للمراجعة، هتشوفها تاني قريب",
        (AddedToReview, Ar) => "أُضيفت إلى المراجعة، ستراها قريبًا",
        (AddedToReview, En) => "Added to review, you'll see it again soon",
        (Listen, ArEg) => "اسمع",
        (Listen, Ar) => "استمع",
        (Listen, En) => "Listen",
        (Unverified, ArEg) => "لسه متراجعتش",
        (Unverified, Ar) => "غير مُراجَعة بعد",
        (Unverified, En) => "Not reviewed yet",
        (NoLocalGloss, ArEg) => {
            "الكلمة دي لسه مش في القاموس المحلي. تقدر تعلّمها لـ\u{2068}Kabar\u{2069} من أي مصدر بتثق فيه."
        }
        (NoLocalGloss, Ar) => {
            "هذه الكلمة ليست في القاموس المحلي بعد. يمكنك تعليمها لـ\u{2068}Kabar\u{2069} من مصدر تثق به."
        }
        (NoLocalGloss, En) => {
            "This word isn't in the local dictionary yet. You can teach it to Kabar from a source you trust."
        }
        (MorphTimeWords, ArEg) => "الزمن في الجملة",
        (MorphTimeWords, Ar) => "الزمن في الجملة",
        (MorphTimeWords, En) => "Time in this sentence",
        (MorphGrammar, ArEg) => "القاعدة",
        (MorphGrammar, Ar) => "القاعدة",
        (MorphGrammar, En) => "Grammar",
        (MorphColours, ArEg) => "معنى الألوان",
        (MorphColours, Ar) => "دلالة الألوان",
        (MorphColours, En) => "What the colours mean",
        (Normalized, ArEg) => "الكلمة كاملة",
        (Normalized, Ar) => "الصيغة الكاملة",
        (Normalized, En) => "Full form",
        (BridgeTitle, ArEg) => "جسر من العربي",
        (BridgeTitle, Ar) => "جسر من العربية",
        (BridgeTitle, En) => "Arabic Bridge",
        (ShiftSame, ArEg) => "نفس المعنى تقريبًا",
        (ShiftSame, Ar) => "المعنى نفسه تقريبًا",
        (ShiftSame, En) => "Same meaning",
        (ShiftNarrowed, ArEg) => "المعنى بقى أضيق",
        (ShiftNarrowed, Ar) => "ضاق المعنى",
        (ShiftNarrowed, En) => "Meaning narrowed",
        (ShiftBroadened, ArEg) => "المعنى بقى أوسع",
        (ShiftBroadened, Ar) => "اتسع المعنى",
        (ShiftBroadened, En) => "Meaning broadened",
        (ShiftShifted, ArEg) => "المعنى اتغير شوية",
        (ShiftShifted, Ar) => "تغيّر المعنى",
        (ShiftShifted, En) => "Meaning shifted",
        (FalseFriend, ArEg) => "صاحب مخادع!",
        (FalseFriend, Ar) => "صديق مخادع!",
        (FalseFriend, En) => "False friend!",
        (SpeechLevels, ArEg | Ar) => "مستويات الاحترام",
        (SpeechLevels, En) => "Speech levels",
        (LevelNgoko, ArEg) => "نجوكو (عادي)",
        (LevelNgoko, Ar) => "نغوكو (عادي)",
        (LevelNgoko, En) => "ngoko (plain)",
        (LevelMadya, ArEg) => "مادْيا (نُص نُص)",
        (LevelMadya, Ar) => "مادْيا (وسط)",
        (LevelMadya, En) => "madya (in-between)",
        (LevelKrama, ArEg) => "كراما (مؤدب)",
        (LevelKrama, Ar) => "كراما (مهذّب)",
        (LevelKrama, En) => "krama (polite)",
        (LevelKramaInggil, ArEg) => "كراما إنغيل (تعظيم)",
        (LevelKramaInggil, Ar) => "كراما إنغيل (تبجيل)",
        (LevelKramaInggil, En) => "krama inggil (honorific)",
        (LevelKramaAndhap, ArEg | Ar) => "كراما أنداب (تواضع)",
        (LevelKramaAndhap, En) => "krama andhap (humble)",
        (TaughtByYou, ArEg) => "إنت اللي علّمته",
        (TaughtByYou, Ar) => "أنت من علّمه",
        (TaughtByYou, En) => "Taught by you",
        (KabarDictionary, ArEg) => "قاموس \u{2068}Kabar\u{2069}",
        (KabarDictionary, Ar) => "قاموس \u{2068}Kabar\u{2069}",
        (KabarDictionary, En) => "Kabar's dictionary",
        (DraftWaiting, ArEg) => "شرحك لسه مسودة، راجعه",
        (DraftWaiting, Ar) => "شرحك ما زال مسودة، راجِعه",
        (DraftWaiting, En) => "Your explanation is a draft. Review it",
        (TeachKabar, ArEg) => "علّم \u{2068}Kabar\u{2069}",
        (TeachKabar, Ar) => "علّم \u{2068}Kabar\u{2069}",
        (TeachKabar, En) => "Teach Kabar",
        (TeachKabarHint, ArEg) => "اديله شرح من أي مصدر بتثق فيه، وهو يكتبلك كارت تراجعه",
        (TeachKabarHint, Ar) => "أعطِه شرحًا من مصدر تثق به، فيكتب لك بطاقة تراجعها",
        (TeachKabarHint, En) => {
            "Give Kabar an explanation from a source you trust; it writes a card for you to review"
        }
        (InIndonesian, ArEg) => "بالإندونيسي",
        (InIndonesian, Ar) => "بالإندونيسية",
        (InIndonesian, En) => "In Indonesian",
        (Helpful, ArEg) => "الشرح ده مفيد",
        (Helpful, Ar) => "الشرح مفيد",
        (Helpful, En) => "Helpful",
        (NotHelpful, ArEg) => "الشرح ده فيه غلط",
        (NotHelpful, Ar) => "الشرح فيه خطأ",
        (NotHelpful, En) => "Something's off",
        (SavePhrase, ArEg | Ar) => "احفظ العبارة",
        (SavePhrase, En) => "Save phrase",
        (PhraseSaved, ArEg) => "العبارة اتحفظت",
        (PhraseSaved, Ar) => "حُفظت العبارة",
        (PhraseSaved, En) => "Phrase saved",
        (RemoveHighlight, ArEg) => "شيل التظليل",
        (RemoveHighlight, Ar) => "أزل التظليل",
        (RemoveHighlight, En) => "Remove highlight",
        (ExplainSentence, ArEg) => "اشرحلي الجملة",
        (ExplainSentence, Ar) => "اشرح الجملة",
        (ExplainSentence, En) => "Explain this sentence",
        (ExplainNeedsCloud, ArEg) => {
            "الشرح الكامل بالذكاء الاصطناعي (معنى الجملة والأسلوب والثقافة) بيشتغل لما توصّل \u{2068}Kabar Cloud\u{2069}. القواعد اللي تحت من مكتبة \u{2068}Kabar\u{2069} نفسها."
        }
        (ExplainNeedsCloud, Ar) => {
            "الشرح الكامل بالذكاء الاصطناعي (معنى الجملة والأسلوب والثقافة) يعمل عند ربط \u{2068}Kabar Cloud\u{2069}. القواعد أدناه من مكتبة \u{2068}Kabar\u{2069} نفسها."
        }
        (ExplainNeedsCloud, En) => {
            "The full AI explanation (meaning, register, culture) works once Kabar Cloud is connected. The grammar below is from Kabar's own library."
        }
        (ExplainResetsLater, ArEg) => {
            "الشرح بالذكاء الاصطناعي خلص رصيده المجاني النهارده، بيرجع بكرة. القواعد اللي تحت من مكتبة \u{2068}Kabar\u{2069} نفسها."
        }
        (ExplainResetsLater, Ar) => {
            "انتهى الرصيد المجاني للشرح بالذكاء الاصطناعي اليوم، ويعود غدًا. القواعد أدناه من مكتبة \u{2068}Kabar\u{2069} نفسها."
        }
        (ExplainResetsLater, En) => {
            "Today's free AI allowance is used up; it's back tomorrow. The grammar below is from Kabar's own library."
        }
        (ExplainUnreachable, ArEg) => {
            "مش قادر أوصل للشرح دلوقتي (النت أو \u{2068}Kabar Home\u{2069}). القواعد اللي تحت شغالة من غير نت."
        }
        (ExplainUnreachable, Ar) => {
            "تعذّر الوصول إلى الشرح الآن (الإنترنت أو \u{2068}Kabar Home\u{2069}). القواعد أدناه تعمل دون اتصال."
        }
        (ExplainUnreachable, En) => {
            "Can't reach the explanation right now (offline, or Kabar Home is asleep). The grammar below works offline."
        }
        (ExplainTryAgain, ArEg) => "الشرح مطلعش مظبوط المرة دي، جرّب تاني بعد شوية. القواعد اللي تحت سليمة.",
        (ExplainTryAgain, Ar) => "لم يخرج الشرح سليمًا هذه المرة، حاول لاحقًا. القواعد أدناه سليمة.",
        (ExplainTryAgain, En) => {
            "The explanation didn't come out right this time; try again in a bit. The grammar below is solid."
        }
        (InOtherWords, ArEg) => "يعني",
        (InOtherWords, Ar) => "المعنى",
        (InOtherWords, En) => "In other words",
        (RegisterChat, ArEg) => "كلام شات",
        (RegisterChat, Ar) => "أسلوب المحادثة",
        (RegisterChat, En) => "chat",
        (RegisterFormal, ArEg) => "رسمي",
        (RegisterFormal, Ar) => "رسمي",
        (RegisterFormal, En) => "formal",
        (RegisterNeutral, ArEg) => "عادي",
        (RegisterNeutral, Ar) => "محايد",
        (RegisterNeutral, En) => "neutral",
        (RegisterSlang, ArEg) => "عامية شبابي",
        (RegisterSlang, Ar) => "عامية",
        (RegisterSlang, En) => "slang",
        (RegisterKrama, ArEg) => "كراما (مؤدب)",
        (RegisterKrama, Ar) => "كراما (مهذّب)",
        (RegisterKrama, En) => "krama (polite)",
        (RegisterNgoko, ArEg) => "نجوكو (بين الأصحاب)",
        (RegisterNgoko, Ar) => "نغوكو (بين المقرّبين)",
        (RegisterNgoko, En) => "ngoko (casual)",
        (RegisterMixed, ArEg) => "خليط",
        (RegisterMixed, Ar) => "مختلط",
        (RegisterMixed, En) => "mixed",
        (CultureNote, ArEg) => "لمسة ثقافية",
        (CultureNote, Ar) => "ملاحظة ثقافية",
        (CultureNote, En) => "Culture note",
        (GrammarInSentence, ArEg) => "القواعد اللي في الجملة",
        (GrammarInSentence, Ar) => "القواعد في هذه الجملة",
        (GrammarInSentence, En) => "Grammar in this sentence",
        (GrammarInText, ArEg) => "القواعد اللي في النص",
        (GrammarInText, Ar) => "القواعد في هذا النص",
        (GrammarInText, En) => "Grammar in this text",
        (GrammarForWord, ArEg) => "القاعدة هنا",
        (GrammarForWord, Ar) => "القاعدة هنا",
        (GrammarForWord, En) => "Grammar here",
        (GrammarLearned, ArEg) => "اتعلمتها",
        (GrammarLearned, Ar) => "تعلّمتَها",
        (GrammarLearned, En) => "Learned",
        (GrammarAtLevel, ArEg) => "في مستواك",
        (GrammarAtLevel, Ar) => "في مستواك",
        (GrammarAtLevel, En) => "At your level",
        (GrammarNotYet, ArEg) => "لسه ماوصلتلهاش",
        (GrammarNotYet, Ar) => "لم تُدرَّس بعد",
        (GrammarNotYet, En) => "Not taught yet",
        (UrlFetching, ArEg) => "بجيب الصفحة…",
        (UrlFetching, Ar) => "جارٍ جلب الصفحة…",
        (UrlFetching, En) => "Fetching the page…",
        (UrlImportFailed, ArEg) => "مقدرتش أقرا الصفحة دي. انسخ النص نفسه والصقه هنا وهيشتغل على طول.",
        (UrlImportFailed, Ar) => "تعذّرت قراءة هذه الصفحة. انسخ النص نفسه والصقه هنا.",
        (UrlImportFailed, En) => "Couldn't read that page. Copy the text itself and paste it here instead.",
        (OcrWorking, ArEg) => "بقرا الصورة…",
        (OcrWorking, Ar) => "جارٍ قراءة الصورة…",
        (OcrWorking, En) => "Reading the image…",
        (WaitWordPrompt, ArEg) => "وإنت مستني، فاكر دي؟",
        (WaitWordPrompt, Ar) => "أثناء الانتظار، هل تتذكر هذه؟",
        (WaitWordPrompt, En) => "While you wait, remember this one?",
        (OcrFailed, ArEg) => "معرفتش أقرا الصورة دي. جرّب صورة أوضح أو الصق الكلام بنفسك.",
        (OcrFailed, Ar) => "تعذّرت قراءة الصورة. جرّب صورة أوضح أو الصق النص.",
        (OcrFailed, En) => "Couldn't read that image. Try a sharper one, or paste the text.",
        (OcrUnavailable, ArEg) => "قراءة الصور لسه مش شغالة على الجهاز ده. الصق الكلام بدلها.",
        (OcrUnavailable, Ar) => "قراءة الصور غير متاحة على هذا الجهاز بعد. الصق النص بدلًا منها.",
        (OcrUnavailable, En) => "Reading images isn't available on this device yet. Paste the text instead.",
        (PickFailed, ArEg) => {
            "معرفتش أفتح الصورة دي. جرّب تختار سكرين شوت تاني، أو شيّرها لـ \u{2068}Kabar\u{2069} من المعرض."
        }
        (PickFailed, Ar) => {
            "تعذّر فتح هذه الصورة. جرّب لقطة شاشة أخرى، أو شاركها مع \u{2068}Kabar\u{2069} من المعرض."
        }
        (PickFailed, En) => {
            "Couldn't open that image. Try another screenshot, or share it to Kabar from your gallery."
        }
        (ScreenshotEmpty, ArEg) => "ملقيتش كلام في الصورة دي.",
        (ScreenshotEmpty, Ar) => "لم أجد نصًا في الصورة.",
        (ScreenshotEmpty, En) => "No text found in that image.",
        (ShareFirstImageOnly, ArEg) => "بنقرا أول صورة بس. ابعت الباقي واحدة واحدة.",
        (ShareFirstImageOnly, Ar) => "نقرأ الصورة الأولى فقط. أرسل البقية واحدة تلو الأخرى.",
        (ShareFirstImageOnly, En) => "Reading the first image only. Share the others one at a time.",
        (SharedFrom, ArEg) => "اتبعت من",
        (SharedFrom, Ar) => "مُشارَك من",
        (SharedFrom, En) => "Shared from",
        (Close, ArEg) => "اقفل",
        (Close, Ar) => "أغلق",
        (Close, En) => "Close",
        (SheetResize, ArEg) => "اسحب عشان تكبّر أو تصغّر، أو استخدم الأسهم",
        (SheetResize, Ar) => "اسحب لتكبير اللوحة أو تصغيرها، أو استخدم الأسهم",
        (SheetResize, En) => "Drag to resize, or use the arrow keys",
        (WordSheet, ArEg) => "تفاصيل الكلمة",
        (WordSheet, Ar) => "تفاصيل الكلمة",
        (WordSheet, En) => "Word details",
        (Me, ArEg | Ar) => "أنا",
        (Me, En) => "Me",
        (PenHint, ArEg) => {
            "بالقلم: قرّب عشان تشوف المعنى، المس الكلمة تفتح الكارت، لف دايرة حوالين عبارة، أو خط تحتها تحفظها."
        }
        (PenHint, Ar) => {
            "بالقلم: قرّب لترى المعنى، المس الكلمة لفتح البطاقة، ارسم دائرة حول عبارة، أو خطًا عليها لحفظها."
        }
        (PenHint, En) => {
            "With the pen: hover to peek, tap a word for its card, loop a phrase to look it up, or stroke across it to save it."
        }
        (UnggahTitle, _) => "Unggah-Ungguh",
        (UnggahBlurb, ArEg) => "اتكلم جاوي مع صاحبك وتيتا والمدرّس، وشوف مقياس الاحترام.",
        (UnggahBlurb, Ar) => "تحدّث بالجاوية مع صديق وجدّة ومعلّم، وراقب مقياس الاحترام.",
        (UnggahBlurb, En) => {
            "Talk Javanese with a friend, a grandmother and a teacher, and watch the Respect Meter."
        }
        (UnggahOpensAt, ArEg) => "بيفتح في",
        (UnggahOpensAt, Ar) => "يُفتح في",
        (UnggahOpensAt, En) => "Opens at",
        (UnggahTurn, ArEg) => "دور",
        (UnggahTurn, Ar) => "الدور",
        (UnggahTurn, En) => "Turn",
        (UnggahFits, ArEg) => "في الجون",
        (UnggahFits, Ar) => "في محلّها تمامًا",
        (UnggahFits, En) => "Just right",
        (UnggahTooPlain, ArEg) => "سادة شوية على الشخص ده",
        (UnggahTooPlain, Ar) => "عادية قليلًا على هذا الشخص",
        (UnggahTooPlain, En) => "A little plain for this person",
        (UnggahTooStiff, ArEg) => "رسمية شوية على الموقف",
        (UnggahTooStiff, Ar) => "رسمية قليلًا على الموقف",
        (UnggahTooStiff, En) => "A little stiff for the moment",
        (UnggahSelfRaised, ArEg) => "كلمة التبجيل دي ليهم، مش ليك",
        (UnggahSelfRaised, Ar) => "لفظ التبجيل لهم، لا لك",
        (UnggahSelfRaised, En) => "Honour words are for them, not for you",
        (UnggahScenes, ArEg) => "مشاهد تانية",
        (UnggahScenes, Ar) => "مشاهد أخرى",
        (UnggahScenes, En) => "Other scenes",
        (UnggahEndGreat, ArEg) => "\u{2068}Sopan!\u{2069} كلامك كان في محلّه، وحسّوا باحترامك.",
        (UnggahEndGreat, Ar) => "\u{2068}Sopan!\u{2069} كان كلامك في محله، وشعروا باحترامك.",
        (UnggahEndGreat, En) => "Sopan! Your words fit, and they felt respected.",
        (UnggahEndGood, ArEg) => "حلو قوي. كام لمسة كمان وتبقى مظبوطة خالص.",
        (UnggahEndGood, Ar) => "جيد جدًا. بضع لمسات أخرى وتصبح متقنًا.",
        (UnggahEndGood, En) => "Really good. A few more touches and it'll be spot on.",
        (UnggahEndGentle, ArEg) => {
            "مستويات الكلام بتاخد وقت، حتى مع الجاويين نفسهم. اقرا الشرح تاني وجرّب المشهد كمان مرة."
        }
        (UnggahEndGentle, Ar) => {
            "مستويات الكلام تحتاج وقتًا، حتى عند الجاويين أنفسهم. اقرأ الشرح مجددًا وجرّب المشهد مرة أخرى."
        }
        (UnggahEndGentle, En) => {
            "Speech levels take time, even for Javanese speakers. Read the notes again and give the scene another go."
        }
        (PairsTitle, ArEg | Ar) => "\u{2068}d/dh · t/th\u{2069}",
        (PairsTitle, En) => "d/dh · t/th",
        (PairsBlurb, ArEg) => {
            "فرّق بين \u{2068}d\u{2069} و\u{2068}dh\u{2069}، و\u{2068}t\u{2069} و\u{2068}th\u{2069} في الجاوي."
        }
        (PairsBlurb, Ar) => {
            "ميّز بين \u{2068}d\u{2069} و\u{2068}dh\u{2069}، و\u{2068}t\u{2069} و\u{2068}th\u{2069} في الجاوية."
        }
        (PairsBlurb, En) => "Tell Javanese d from dh, and t from th.",
        (PairsBoth, ArEg) => "الاتنين",
        (PairsBoth, Ar) => "كلاهما",
        (PairsBoth, En) => "Both",
        (PairsListenMode, ArEg) => "اسمع",
        (PairsListenMode, Ar) => "استمع",
        (PairsListenMode, En) => "Listen",
        (PairsReadMode, ArEg) => "اقرا",
        (PairsReadMode, Ar) => "اقرأ",
        (PairsReadMode, En) => "Read",
        (PairsNoJavaneseVoice, ArEg) => {
            "الجهاز ده مفيهوش صوت جاوي، فهنلعب بالقراية: تشوف المعنى وتختار الكتابة. (الصوت الإندونيسي مبيعرفش يقول \u{2068}dh\u{2069} و\u{2068}th\u{2069}، فمش هنستعمله هنا.)"
        }
        (PairsNoJavaneseVoice, Ar) => {
            "لا يوجد صوت جاوي على هذا الجهاز، فسنلعب بالقراءة: ترى المعنى وتختار الكتابة. (الصوت الإندونيسي لا ينطق \u{2068}dh\u{2069} و\u{2068}th\u{2069}، فلن نستخدمه هنا.)"
        }
        (PairsNoJavaneseVoice, En) => {
            "This device has no Javanese voice, so we'll play by reading: see the meaning, pick the spelling. (Indonesian voices can't make dh and th, so we don't use them here.)"
        }
        (PairsHear, ArEg) => "اسمعها",
        (PairsHear, Ar) => "استمع إليها",
        (PairsHear, En) => "Hear it",
        (PairsWhichHeard, ArEg) => "سمعت أنهي كلمة؟",
        (PairsWhichHeard, Ar) => "أيّ كلمة سمعت؟",
        (PairsWhichHeard, En) => "Which word did you hear?",
        (PairsWhichSpelling, ArEg) => "أنهي كتابة صح؟",
        (PairsWhichSpelling, Ar) => "أيّ الكتابتين صحيحة؟",
        (PairsWhichSpelling, En) => "Which spelling is it?",
        (PairsJavanese, ArEg) => "جاوي",
        (PairsJavanese, Ar) => "جاوية",
        (PairsJavanese, En) => "Javanese",
        (PairsIndonesian, ArEg) => "إندونيسي",
        (PairsIndonesian, Ar) => "إندونيسية",
        (PairsIndonesian, En) => "Indonesian",
        (PronModeOn, ArEg) => "ظاهر على طول",
        (PronModeOn, Ar) => "ظاهر دائمًا",
        (PronModeOn, En) => "Always shown",
        (PronModeTap, ArEg) => "لما أدوس",
        (PronModeTap, Ar) => "عند النقر",
        (PronModeTap, En) => "On tap",
        (PronModeOff, ArEg) => "مقفول",
        (PronModeOff, Ar) => "مُطفأ",
        (PronModeOff, En) => "Off",
        (PronListen, ArEg) => "اسمع النطق الأصلي",
        (PronListen, Ar) => "استمع إلى النطق الأصلي",
        (PronListen, En) => "Hear it said",
        (PronTitle, ArEg) => "النطق بالحروف العربي",
        (PronTitle, Ar) => "النطق بالحروف العربية",
        (PronTitle, En) => "Pronunciation in Arabic script",
        (PronShow, ArEg) => "اتنطق إزاي؟",
        (PronShow, Ar) => "كيف تُنطق؟",
        (PronShow, En) => "How to say it?",
        (PronHowToRead, ArEg) => "إزاي أقرا التعريب؟",
        (PronHowToRead, Ar) => "كيف أقرأ النطق العربي؟",
        (PronHowToRead, En) => "How to read the Arabic pronunciation",
        (PronGuideIntro, ArEg) => {
            "تحت كل كلمة إندونيسي أو جاوي هتلاقي نطقها بالحروف العربي بالتشكيل. كل الحروف عربي أصلي، بس فيه أصوات مش في العربي ليها كتابة ثابتة؛ دي اللي محتاج تاخد بالك منها:"
        }
        (PronGuideIntro, Ar) => {
            "تحت كل كلمة إندونيسية أو جاوية نطقها بالحروف العربية مشكولة. والحروف كلها عربية أصلية، وهذه الأصوات التي لها كتابة خاصة:"
        }
        (PronGuideIntro, En) => {
            "Under each Indonesian or Javanese word you'll see how to say it, in fully vowelled Arabic script. Every letter is plain Arabic; these are the sounds with a special spelling:"
        }
        (PronGuideNote, ArEg) => {
            "الراء ملفوفة زي العربي، والهاء في آخر الكلمة بتتقال خفيفة. التعريب تقريب يساعدك تبدأ: اسمع الصوت، ولو اختلفوا صدّق ودنك. نطق الجاوي لسه محتاج مراجعة."
        }
        (PronGuideNote, Ar) => {
            "الراء مكررة كالعربية، والهاء في آخر الكلمة خفيفة. النطق المكتوب تقريب للبداية: استمع إلى الصوت، وإن اختلفا فالعبرة بالسمع. نطق الجاوية لم يُراجع بعد."
        }
        (PronGuideNote, En) => {
            "r is rolled as in Arabic, and a final h is light. The script is an approximation to get you started: listen to the audio and trust your ear when they differ. Javanese pronunciations aren't reviewed yet."
        }
        (SaveFailed, ArEg) => {
            "فيه إعدادات ما اتحفظتش. اختياراتك شغالة دلوقتي، بس ممكن تضيع لما تقفل التطبيق."
        }
        (SaveFailed, Ar) => "تعذّر حفظ بعض الإعدادات. اختياراتك مطبّقة الآن، وقد تُفقد عند إغلاق التطبيق.",
        (SaveFailed, En) => {
            "Some preferences could not be saved. Your choices apply now but may be lost when you close the app."
        }
        (SaveRetry, ArEg) => "حاول تحفظ تاني",
        (SaveRetry, Ar) => "إعادة محاولة الحفظ",
        (SaveRetry, En) => "Retry saving",
        (WebOtherTab, ArEg) => {
            "\u{2068}Kabar\u{2069} مفتوح في تاب تانية، فالتاب دي وقفت تحفظ. اعمل تحديث للصفحة عشان تكمّل هنا."
        }
        (WebOtherTab, Ar) => {
            "\u{2068}Kabar\u{2069} مفتوح في تبويب آخر، فتوقّف هذا التبويب عن الحفظ. حدّث الصفحة لتكمل هنا."
        }
        (WebOtherTab, En) => {
            "Kabar is open in another tab, so this one stopped saving. Reload to keep learning here."
        }
        (WebSaveFailed, ArEg) => {
            "المتصفح ماقبلش يحفظ تقدّمك دلوقتي. هنحاول تاني لوحدنا؛ ولو فضلت الرسالة دي، اعمل تصدير من الإعدادات."
        }
        (WebSaveFailed, Ar) => {
            "رفض المتصفح حفظ تقدّمك الآن. سنحاول مجددًا تلقائيًا؛ وإن بقيت هذه الرسالة فصدّر بياناتك من الإعدادات."
        }
        (WebSaveFailed, En) => {
            "The browser wouldn't keep your progress just now. Kabar keeps trying; if this stays, export your data from Settings."
        }
        (ProblemWrongPassword, ArEg) => "اسم المستخدم أو الباسورد مش مظبوط. جرّب تاني.",
        (ProblemWrongPassword, Ar) => "اسم المستخدم أو كلمة المرور غير صحيحة. حاول مرة أخرى.",
        (ProblemWrongPassword, En) => "That username and password don't match. Try again.",
        (ProblemLocked, ArEg) => {
            "محاولات غلط كتير، فالدخول بالاسم ده واقف شوية. جرّب تاني بعد {n|دقيقة|دقيقتين|دقايق}."
        }
        (ProblemLocked, Ar) => {
            "محاولات خاطئة كثيرة، فالدخول بهذا الاسم متوقف قليلًا. حاول مجددًا بعد {n|دقيقة|دقيقتين|دقائق|دقيقة}."
        }
        (ProblemLocked, En) => "Too many wrong tries, so this username is paused. Try again in {n} min.",
        (ProblemTaken, ArEg) => "اسم المستخدم ده محجوز. اختار اسم تاني.",
        (ProblemTaken, Ar) => "اسم المستخدم هذا محجوز. اختر اسمًا آخر.",
        (ProblemTaken, En) => "That username is taken. Pick another.",
        (ProblemBadUsername, ArEg) => "اسم المستخدم من ٣ لـ ٣٢: حروف إنجليزي وأرقام و . _ - بس.",
        (ProblemBadUsername, Ar) => "اسم المستخدم من ٣ إلى ٣٢: حروف إنجليزية وأرقام و . _ - فقط.",
        (ProblemBadUsername, En) => "A username is 3 to 32 of a–z, 0–9 and . _ - only.",
        (ProblemShortPassword, ArEg) => "الباسورد لازم يبقى ١٠ حروف على الأقل.",
        (ProblemShortPassword, Ar) => "يجب أن تكون كلمة المرور ١٠ أحرف على الأقل.",
        (ProblemShortPassword, En) => "A password needs at least 10 characters.",
        (ProblemMismatch, ArEg) => "الباسوردين مش زي بعض.",
        (ProblemMismatch, Ar) => "كلمتا المرور غير متطابقتين.",
        (ProblemMismatch, En) => "The two passwords differ.",
    }
}

/// A gloss's part of speech (`noun`, `proper_noun`, …) in the UI language.
/// Unknown tags come back as written.
pub fn part_of_speech(lang: UiLanguage, pos: &str) -> &str {
    use UiLanguage::*;
    let [arz, ar, en] = match pos {
        "noun" => ["اسم", "اسم", "noun"],
        "proper_noun" | "proper noun" => ["اسم علم", "اسم علم", "proper noun"],
        "verb" => ["فعل", "فعل", "verb"],
        "adjective" => ["صفة", "صفة", "adjective"],
        "adverb" => ["ظرف", "ظرف", "adverb"],
        "pronoun" => ["ضمير", "ضمير", "pronoun"],
        "preposition" => ["حرف جر", "حرف جر", "preposition"],
        "conjunction" => ["أداة ربط", "حرف عطف أو ربط", "conjunction"],
        "particle" => ["أداة", "أداة", "particle"],
        "numeral" => ["رقم", "عدد", "numeral"],
        "classifier" => ["كلمة عدّ", "مُصنِّف عددي", "classifier"],
        "determiner" => ["محدِّد", "محدِّد", "determiner"],
        "interjection" => ["كلمة تعجب", "أداة تعجب", "interjection"],
        "phrase" => ["تعبير", "عبارة", "phrase"],
        _ => return pos,
    };
    match lang {
        ArEg => arz,
        Ar => ar,
        En => en,
    }
}

#[cfg(test)]
mod grammar_tests {
    use super::*;
    use crate::stats::text::{S, st};

    const NB: char = '\u{a0}';

    fn msg(lang: UiLanguage, m: impl Text, n: &str) -> String {
        fill(lang, t(lang, m), &[("n", n), ("rows", n)])
    }

    /// Counts agree with their nouns: 1 and 2 by the noun alone, 3–10 the
    /// plural, 11 and up the singular (Mohamed, 2026-11-02).
    #[test]
    fn counted_lines_agree_in_arabic() {
        use UiLanguage::*;
        assert_eq!(msg(ArEg, Msg::DiagCount, "1"), "سطر");
        assert_eq!(msg(ArEg, Msg::DiagCount, "2"), "سطرين");
        assert_eq!(msg(ArEg, Msg::DiagCount, "7"), format!("7{NB}سطور"));
        assert_eq!(msg(ArEg, Msg::DiagCount, "12"), format!("12{NB}سطر"));
        assert_eq!(msg(ArEg, Msg::ExportSaved, "٢"), "اتحفظ صفين في:");
        assert_eq!(msg(Ar, Msg::ExportSaved, "15"), format!("حُفظ 15{NB}صفًّا في:"));
        assert_eq!(
            msg(ArEg, crate::review::msg::ReviewMsg::RevBurstHint, "60"),
            format!("أكتر عدد كروت تقدر عليه في 60{NB}ثانية.")
        );
        assert!(
            msg(ArEg, crate::settings::msg::SettingsMsg::RetentionTuned, "5")
                .ends_with(&format!("من 5{NB}مراجعات."))
        );
        assert!(
            msg(ArEg, crate::review::msg::ReviewMsg::RevSeedBody, "2")
                .starts_with("كلمتين إندونيسي أصلها عربي.")
        );
        let stat = |lang, s, args: &[(&str, &str)]| fill(lang, st(lang, s), args);
        assert_eq!(stat(ArEg, S::In30Days, &[("n", "30")]), format!("بعد 30{NB}يوم"));
        assert_eq!(stat(ArEg, S::PeriodWeek, &[("n", "7")]), format!("آخر 7{NB}أيام"));
        assert_eq!(stat(ArEg, S::PeriodMonth, &[("n", "4")]), format!("آخر 4{NB}أسابيع"));
        assert_eq!(
            stat(ArEg, S::RingExplain, &[("total", "2"), ("passed", "1")]),
            "من مراجعتين لكلمات شفتها قبل كده، افتكرت 1."
        );
        assert!(
            stat(
                ArEg,
                S::ForecastSummary,
                &[("n", "30"), ("total", "7"), ("avg", "1"), ("day", "x"), ("peak", "3")]
            )
            .starts_with(&format!("في 30{NB}يوم جايين: 7{NB}مراجعات في المجموع"))
        );
    }

    #[test]
    fn counted_lines_agree_in_english() {
        assert_eq!(msg(UiLanguage::En, Msg::DiagCount, "1"), format!("1{NB}line"));
        assert_eq!(msg(UiLanguage::En, Msg::ExportSaved, "3"), format!("Saved 3{NB}rows to:"));
    }
}

/// Checks any catalog's strings: shared by every catalog in the app.
#[cfg(test)]
pub(crate) mod checks {
    use kabar_core::locale::UiLanguage;

    pub const LANGS: [UiLanguage; 3] = [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En];

    /// A `{name}` or `{name|form|…}` slot.
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Slot {
        pub name: String,
        pub forms: usize,
    }

    /// The template's slots, or why it would render with a stray brace.
    pub fn slots(text: &str) -> Result<Vec<Slot>, String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(i) = rest.find(['{', '}']) {
            if rest.as_bytes()[i] == b'}' {
                return Err("stray '}'".into());
            }
            let after = &rest[i + 1..];
            let close = after.find('}').ok_or("unclosed '{'")?;
            let inner = &after[..close];
            if inner.contains('{') {
                return Err("nested '{'".into());
            }
            let mut parts = inner.split('|');
            let name = parts.next().unwrap_or_default();
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                return Err(format!("bad placeholder {{{inner}}}"));
            }
            let forms: Vec<&str> = parts.collect();
            if forms.iter().any(|f| f.trim().is_empty()) {
                return Err(format!("empty plural form in {{{inner}}}"));
            }
            out.push(Slot { name: name.to_string(), forms: forms.len() });
            rest = &after[close + 1..];
        }
        Ok(out)
    }

    fn arabic_letters(text: &str) -> bool {
        text.chars().any(|c| matches!(c, '\u{0621}'..='\u{064a}'))
    }

    /// Whether `text` would read wrongly in its language: blank, padded, a
    /// broken placeholder, unbalanced bidi isolates, Arabic in English or
    /// no Arabic at all in Arabic (`mixed_ok` skips the script check, for
    /// names such as "Kak Nur" that stay Latin in every language).
    pub fn problems(lang: UiLanguage, text: &str, mixed_ok: bool) -> Vec<String> {
        let mut out = Vec::new();
        if text.trim().is_empty() {
            out.push("blank".to_string());
        }
        if text.trim() != text {
            out.push("leading or trailing space".to_string());
        }
        if let Err(e) = slots(text) {
            out.push(e);
        }
        let opens = text.matches(['\u{2066}', '\u{2067}', '\u{2068}']).count();
        if opens != text.matches('\u{2069}').count() {
            out.push("unbalanced bidi isolates".to_string());
        }
        match lang {
            _ if mixed_ok => {}
            UiLanguage::En if arabic_letters(text) => out.push("Arabic letters in English".to_string()),
            UiLanguage::ArEg | UiLanguage::Ar if !arabic_letters(text) => {
                out.push("no Arabic in an Arabic string".to_string())
            }
            _ => {}
        }
        for slot in slots(text).unwrap_or_default() {
            let ok = matches!(
                (slot.forms, lang),
                (0, _) | (2, UiLanguage::En) | (3 | 4, UiLanguage::ArEg | UiLanguage::Ar)
            );
            if !ok {
                out.push(format!("{{{}}} has {} plural forms", slot.name, slot.forms));
            }
        }
        out
    }

    /// Every key in every language: no problems, the same placeholders in
    /// each, and filled with any count it leaves no brace behind.
    pub fn catalog<K: Copy + std::fmt::Debug>(
        all: &[K],
        text: impl Fn(UiLanguage, K) -> &'static str,
        mixed_ok: impl Fn(K) -> bool,
    ) {
        let mut bad = Vec::new();
        for &key in all {
            let mut names: Vec<Vec<String>> = Vec::new();
            for lang in LANGS {
                let s = text(lang, key);
                for p in problems(lang, s, mixed_ok(key)) {
                    bad.push(format!("{key:?} {}: {p}: {s:?}", lang.tag()));
                }
                let mut n: Vec<String> = slots(s).unwrap_or_default().into_iter().map(|s| s.name).collect();
                n.sort();
                n.dedup();
                for count in ["1", "2", "7", "15", "٣"] {
                    let args: Vec<(&str, &str)> = n.iter().map(|k| (k.as_str(), count)).collect();
                    let filled = super::fill(lang, s, &args);
                    if filled.contains(['{', '}']) {
                        bad.push(format!("{key:?} {}: left a brace with {count}: {filled:?}", lang.tag()));
                    }
                }
                names.push(n);
            }
            if names.iter().any(|n| *n != names[0]) {
                bad.push(format!("{key:?}: placeholders differ between languages: {names:?}"));
            }
        }
        assert!(bad.is_empty(), "{} catalog problems:\n{}", bad.len(), bad.join("\n"));
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::checks::{Slot, problems, slots};
    use super::*;

    /// Keys exempt from the script check: names that stay Latin in Arabic
    /// (Kak Nur, Kabar Pagi, the Javanese game titles), and English that
    /// shows Arabic letters on purpose (د and ت beside d and t).
    fn mixed_script(m: Msg) -> bool {
        use Msg::*;
        matches!(
            m,
            NavCoach
                | NavPenLab
                | TodayTitle
                | CostUnitNeurons
                | StatPelita
                | PagiTitle
                | PasarTitle
                | UnggahTitle
                | PairsTitle
        )
    }

    #[test]
    fn every_message_reads_right_in_every_language() {
        assert!(Msg::ALL.len() > 600, "the catalog walk must see every key");
        checks::catalog(Msg::ALL, t, mixed_script);
    }

    #[test]
    fn the_checks_catch_broken_strings() {
        use UiLanguage::*;
        assert_eq!(
            slots("{n|كلمة|كلمتين|كلمات} من {total}"),
            Ok(vec![Slot { name: "n".into(), forms: 3 }, Slot { name: "total".into(), forms: 0 }])
        );
        for broken in ["a } b", "{n", "{a{b}}", "{N}", "{n||x}"] {
            assert!(slots(broken).is_err(), "{broken}");
        }
        assert!(problems(En, "Saved {n|row|rows}", false).is_empty());
        assert!(problems(ArEg, "اتحفظ {n|صف|صفين|صفوف}", false).is_empty());
        assert_eq!(problems(En, " Hi", false), ["leading or trailing space"]);
        assert_eq!(problems(En, "", false)[0], "blank");
        assert_eq!(problems(En, "Hi يا", false), ["Arabic letters in English"]);
        assert_eq!(problems(Ar, "Kabar", false), ["no Arabic in an Arabic string"]);
        assert!(problems(Ar, "Kak Nur", true).is_empty());
        assert_eq!(problems(Ar, "كلمة \u{2068}kabar", false), ["unbalanced bidi isolates"]);
        assert_eq!(problems(En, "{n|row}", false), ["{n} has 1 plural forms"]);
        assert_eq!(problems(ArEg, "{n|صف|صفين}", false), ["{n} has 2 plural forms"]);
    }

    #[test]
    fn parts_of_speech_are_named_in_each_language() {
        use UiLanguage::*;
        assert_eq!(part_of_speech(ArEg, "noun"), "اسم");
        assert_eq!(part_of_speech(Ar, "numeral"), "عدد");
        assert_eq!(part_of_speech(ArEg, "numeral"), "رقم");
        assert_eq!(part_of_speech(En, "proper noun"), "proper noun");
        assert_eq!(part_of_speech(ArEg, "proper_noun"), "اسم علم");
        assert_eq!(part_of_speech(En, "xyz-tag"), "xyz-tag", "unknown tags come back as written");
        for pos in [
            "noun",
            "verb",
            "adjective",
            "adverb",
            "pronoun",
            "preposition",
            "conjunction",
            "particle",
            "numeral",
            "classifier",
            "determiner",
            "interjection",
            "phrase",
        ] {
            assert_eq!(part_of_speech(En, pos), pos);
            for lang in [ArEg, Ar] {
                let ar = part_of_speech(lang, pos);
                assert!(problems(lang, ar, false).is_empty(), "{pos} {ar}");
            }
        }
    }

    /// The four Arabic count forms (Mohamed, 2026-11-02) for a few counted
    /// messages, Egyptian and MSA, ASCII and Eastern digits.
    #[test]
    fn counts_follow_the_arabic_number_rule() {
        use UiLanguage::*;
        const NB: char = '\u{a0}';
        let f = |lang, m, n: &str| fill(lang, t(lang, m), &[("n", n), ("rows", n)]);
        assert_eq!(f(ArEg, Msg::ExportSaved, "1"), "اتحفظ صف في:");
        assert_eq!(f(ArEg, Msg::ExportSaved, "3"), format!("اتحفظ 3{NB}صفوف في:"));
        assert_eq!(f(ArEg, Msg::ExportSaved, "11"), format!("اتحفظ 11{NB}صف في:"));
        assert_eq!(f(ArEg, Msg::ExportSaved, "١٠٠"), format!("اتحفظ ١٠٠{NB}صف في:"));
        assert_eq!(f(Ar, Msg::ExportSaved, "1"), "حُفظ صفّ في:");
        assert_eq!(f(Ar, Msg::ExportSaved, "2"), "حُفظ صفّان في:");
        assert_eq!(f(Ar, Msg::ExportSaved, "٤"), format!("حُفظ ٤{NB}صفوف في:"));
        assert_eq!(f(En, Msg::ExportSaved, "1"), format!("Saved 1{NB}row to:"));
    }
}
