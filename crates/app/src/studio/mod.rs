//! Tablet Studio (M9, spec §4, PLAN §17): the Notebook, Gambar!, the
//! mind-map generator and the Listening Companion, all on one ink canvas
//! ([`ink::InkCanvas`]). Write-mode lessons use [`pad::InkPad`] inside the
//! Lesson Player.

pub mod aksara;
pub mod gambar;
pub mod ink;
pub mod listening;
pub mod mindmap;
pub mod notebook;
pub mod pad;
pub mod vision;

use dioxus::prelude::*;
use kabar_core::ai::{AiError, FriendlyReason};
use kabar_core::locale::UiLanguage;
use kabar_protocol::content::Lang;

use crate::Ui;
use crate::reader::glossary::WordLang;

const STUDIO_CSS: &str = include_str!("../../assets/studio.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Notebook,
    Gambar,
    MindMap,
    Listening,
    Write,
    Aksara,
}

/// Pen writing is paused on every device (SPEC §23, the learner's call):
/// the Studio shows only the mind map. The writing tools stay in the code;
/// flip this to bring them back.
const WRITING_PAUSED: bool = true;

impl Tab {
    fn is_writing(self) -> bool {
        !matches!(self, Self::MindMap)
    }
}

#[component]
pub fn StudioScreen() -> Element {
    let ui = use_context::<Ui>();
    let mut tab = use_signal(|| Tab::MindMap);
    let l = ui.lang();
    let tabs = [
        (Tab::Notebook, T::Notebook),
        (Tab::Gambar, T::Gambar),
        (Tab::MindMap, T::MindMap),
        (Tab::Listening, T::Listening),
        (Tab::Write, T::WriteTab),
        (Tab::Aksara, T::Aksara),
    ]
    .into_iter()
    .filter(|(t, _)| !WRITING_PAUSED || !t.is_writing())
    .collect::<Vec<_>>();
    let shown = tabs.len();
    rsx! {
        style { {STUDIO_CSS} }
        div { class: "studio",
            div { class: "studio-tabs", role: "tablist", hidden: shown < 2,
                for (t, label) in tabs {
                    button {
                        key: "{label:?}",
                        role: "tab",
                        aria_selected: if tab() == t { "true" } else { "false" },
                        aria_pressed: if tab() == t { "true" } else { "false" },
                        onclick: move |_| tab.set(t),
                        "{tr(l, label)}"
                    }
                }
            }
            match tab() {
                Tab::Notebook => rsx! { notebook::NotebookScreen {} },
                Tab::Gambar => rsx! { gambar::GambarScreen {} },
                Tab::MindMap => rsx! { mindmap::MindMapScreen {} },
                Tab::Listening => rsx! { listening::ListeningScreen {} },
                Tab::Write => rsx! { pad::DictationScreen {} },
                Tab::Aksara => rsx! { aksara::AksaraScreen {} },
            }
        }
    }
}

/// The tool labels for [`ink::InkTools`].
pub fn tool_labels(l: UiLanguage) -> [&'static str; 4] {
    [tr(l, T::Pen), tr(l, T::Highlighter), tr(l, T::Brush), tr(l, T::Eraser)]
}

pub fn word_lang(l: Lang) -> WordLang {
    match l {
        Lang::Id => WordLang::Indonesian,
        Lang::Jv => WordLang::Javanese,
    }
}

/// The friendly line for an AI call that gave no answer.
pub fn ai_problem(e: &AiError) -> T {
    friendly(e.friendly())
}

pub fn friendly(f: FriendlyReason) -> T {
    match f {
        FriendlyReason::ResetsLater => T::ResetsLater,
        FriendlyReason::Unreachable => T::Unreachable,
        FriendlyReason::NotAvailable => T::NotConnected,
        FriendlyReason::TryAgain => T::TryAgain,
    }
}

/// A new UUIDv7 (sorts by creation time).
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    Notebook,
    Gambar,
    MindMap,
    Listening,
    Pen,
    Highlighter,
    Brush,
    Eraser,
    // Notebook
    MyNotebook,
    NewNotebook,
    NewPage,
    Page,
    Pages,
    AddCard,
    CardPlaceholder,
    Search,
    NoResults,
    ReadWriting,
    Reading,
    Lines,
    NoLines,
    TypeWhatYouWrote,
    Save,
    Remove,
    Listen,
    // AI
    NotConnected,
    ResetsLater,
    Unreachable,
    TryAgain,
    // Gambar!
    GambarIntro,
    Start,
    DrawThis,
    SecondsLeft,
    Done,
    Guessing,
    Got,
    Close,
    Missed,
    Unclear,
    KabarSaw,
    SelfCheck,
    Yes,
    NotReally,
    NextRound,
    Score,
    // Mind map
    MapIntro,
    Topic,
    Make,
    Making,
    Unverified,
    Maps,
    // Listening Companion
    ListenIntro,
    HeardIt,
    NoSuggestions,
    AddReview,
    TeachIt,
    Saved,
    ScribbleHere,
    // Pad
    WriteTab,
    DictationIntro,
    PlayAgain,
    WriteHere,
    Check,
    Clear,
    CorrectForm,
    // Aksara Jawa
    Aksara,
    AksaraIntro,
    Letters,
    Signs,
    Finals,
    Words,
    Watch,
    Trace,
    WriteFree,
    Replay,
    OrderUnverified,
    CheckShape,
    AskTeacher,
    TeacherSays,
    TraceDone,
    WriteDone,
    TryWriting,
    NextLetter,
    TryAgainHere,
    NothingYet,
}

pub fn tr(lang: UiLanguage, msg: T) -> &'static str {
    use T::*;
    use UiLanguage::*;
    match (msg, lang) {
        (Notebook, ArEg | Ar) => "الكشكول",
        (Notebook, En) => "Notebook",
        (Gambar, _) => "Gambar!",
        (MindMap, ArEg) => "خريطة كلمات",
        (MindMap, Ar) => "خريطة ذهنية",
        (MindMap, En) => "Mind map",
        (Listening, ArEg) => "رفيق السماع",
        (Listening, Ar) => "رفيق الاستماع",
        (Listening, En) => "Listening Companion",
        (Pen, ArEg | Ar) => "قلم",
        (Pen, En) => "Pen",
        (Highlighter, ArEg) => "هايلايتر",
        (Highlighter, Ar) => "قلم تمييز",
        (Highlighter, En) => "Highlighter",
        (Brush, ArEg | Ar) => "فرشاة",
        (Brush, En) => "Brush",
        (Eraser, ArEg) => "أستيكة",
        (Eraser, Ar) => "ممحاة",
        (Eraser, En) => "Eraser",

        (MyNotebook, ArEg) => "كشكولي",
        (MyNotebook, Ar) => "دفتري",
        (MyNotebook, En) => "My notebook",
        (NewNotebook, ArEg) => "كشكول جديد",
        (NewNotebook, Ar) => "دفتر جديد",
        (NewNotebook, En) => "New notebook",
        (NewPage, ArEg | Ar) => "صفحة جديدة",
        (NewPage, En) => "New page",
        (Page, ArEg | Ar) => "صفحة",
        (Page, En) => "Page",
        (Pages, ArEg | Ar) => "الصفحات",
        (Pages, En) => "Pages",
        (AddCard, ArEg) => "حط كارت كلمة",
        (AddCard, Ar) => "أضف بطاقة كلمة",
        (AddCard, En) => "Add a word card",
        (CardPlaceholder, ArEg | Ar) => "pasar، makan، sinau…",
        (CardPlaceholder, En) => "pasar, makan, sinau…",
        (Search, ArEg) => "دوّر في الكشاكيل",
        (Search, Ar) => "ابحث في الدفاتر",
        (Search, En) => "Search your notebooks",
        (NoResults, ArEg) => "مفيش حاجة بالاسم ده لسه.",
        (NoResults, Ar) => "لا نتائج بعد.",
        (NoResults, En) => "Nothing with that yet.",
        (ReadWriting, ArEg) => "اقرا خطي",
        (ReadWriting, Ar) => "اقرأ خطي",
        (ReadWriting, En) => "Read my writing",
        (Reading, ArEg) => "بقرا…",
        (Reading, Ar) => "جارٍ القراءة…",
        (Reading, En) => "Reading…",
        (Lines, ArEg) => "السطور اللي كتبتها",
        (Lines, Ar) => "الأسطر المكتوبة",
        (Lines, En) => "Your lines",
        (NoLines, ArEg) => "اكتب بالقلم، والسطور هتظهر هنا عشان تقدر تدوّر فيها بعدين.",
        (NoLines, Ar) => "اكتب بالقلم، وستظهر الأسطر هنا لتبحث فيها لاحقًا.",
        (NoLines, En) => "Write with the pen; your lines show up here so you can search them later.",
        (TypeWhatYouWrote, ArEg) => "اكتب اللي كتبته بإيدك",
        (TypeWhatYouWrote, Ar) => "اكتب ما خططته بيدك",
        (TypeWhatYouWrote, En) => "Type what you wrote",
        (Save, ArEg | Ar) => "حفظ",
        (Save, En) => "Save",
        (Remove, ArEg) => "شيل",
        (Remove, Ar) => "إزالة",
        (Remove, En) => "Remove",
        (Listen, ArEg) => "اسمع",
        (Listen, Ar) => "استمع",
        (Listen, En) => "Listen",

        (NotConnected, ArEg) => {
            "\u{2068}Kabar\u{2069} لسه مش شايف الخط والرسم على الجهاز ده. ده محتاج Kabar Cloud أو التعرف على الخط من الأندرويد، وجايين. لحد كده تقدر تكتب اللي كتبته."
        }
        (NotConnected, Ar) => {
            "لا يستطيع \u{2068}Kabar\u{2069} قراءة الخط والرسم على هذا الجهاز بعد؛ يحتاج ذلك إلى Kabar Cloud أو التعرف على الخط في أندرويد، وهما في الطريق. يمكنك حتى ذلك الحين كتابة ما خططته."
        }
        (NotConnected, En) => {
            "Kabar can't read ink on this device yet: that needs Kabar Cloud or Android's handwriting recognizer, both on the way. Until then you can type what you wrote."
        }
        (ResetsLater, ArEg) => "الحصة المجانية للنهارده خلصت. بكرة ترجع تاني.",
        (ResetsLater, Ar) => "انتهت الحصة المجانية لليوم، وتعود غدًا.",
        (ResetsLater, En) => "Today's free quota is used up; it comes back tomorrow.",
        (Unreachable, ArEg) => "مش قادر أوصل للسيرفر دلوقتي. جرّب تاني لما النت يرجع.",
        (Unreachable, Ar) => "تعذّر الوصول إلى الخادم الآن. حاول مجددًا عند عودة الاتصال.",
        (Unreachable, En) => "Can't reach the server right now. Try again when you're back online.",
        (TryAgain, ArEg) => "حصلت لخبطة. جرّب تاني.",
        (TryAgain, Ar) => "حدث خطأ. حاول مجددًا.",
        (TryAgain, En) => "Something went wrong. Try again.",

        (GambarIntro, ArEg) => "ارسم الكلمة في دقيقة، و \u{2068}Kabar\u{2069} يحاول يخمّن هي إيه.",
        (GambarIntro, Ar) => "ارسم الكلمة في دقيقة، ويحاول \u{2068}Kabar\u{2069} تخمينها.",
        (GambarIntro, En) => "Draw the word in a minute and Kabar tries to guess it.",
        (Start, ArEg) => "يلا نبدأ",
        (Start, Ar) => "ابدأ",
        (Start, En) => "Start",
        (DrawThis, ArEg) => "ارسم:",
        (DrawThis, Ar) => "ارسم:",
        (DrawThis, En) => "Draw:",
        (SecondsLeft, ArEg) => "ثانية فاضلة",
        (SecondsLeft, Ar) => "ثانية متبقية",
        (SecondsLeft, En) => "seconds left",
        (Done, ArEg) => "خلصت",
        (Done, Ar) => "انتهيت",
        (Done, En) => "Done",
        (Guessing, ArEg) => "\u{2068}Kabar\u{2069} بيبص على رسمتك…",
        (Guessing, Ar) => "\u{2068}Kabar\u{2069} ينظر إلى رسمتك…",
        (Guessing, En) => "Kabar is looking at your drawing…",
        (Got, ArEg) => "\u{2068}Kabar\u{2069} عرفها! برافو.",
        (Got, Ar) => "عرفها \u{2068}Kabar\u{2069}! أحسنت.",
        (Got, En) => "Kabar got it! Nice.",
        (Close, ArEg) => "قرّب جدًا، كانت في باله.",
        (Close, Ar) => "اقترب جدًا، كانت ضمن تخميناته.",
        (Close, En) => "So close: it was on Kabar's list.",
        (Missed, ArEg) => "\u{2068}Kabar\u{2069} شاف حاجة تانية خالص، وده نص المتعة.",
        (Missed, Ar) => "رأى \u{2068}Kabar\u{2069} شيئًا آخر، وهذه نصف المتعة.",
        (Missed, En) => "Kabar saw something else entirely, which is half the fun.",
        (Unclear, ArEg) => "\u{2068}Kabar\u{2069} مش قادر يحدد. جرّب خطوط أوضح المرة الجاية.",
        (Unclear, Ar) => "لم يستطع \u{2068}Kabar\u{2069} التحديد. جرّب خطوطًا أوضح المرة القادمة.",
        (Unclear, En) => "Kabar couldn't tell. Try bolder lines next time.",
        (KabarSaw, ArEg) => "\u{2068}Kabar\u{2069} شاف:",
        (KabarSaw, Ar) => "رأى \u{2068}Kabar\u{2069}:",
        (KabarSaw, En) => "Kabar saw:",
        (SelfCheck, ArEg) => {
            "\u{2068}Kabar\u{2069} لسه مش بيشوف الرسم على الجهاز ده. بصّ على رسمتك: شبه الكلمة؟"
        }
        (SelfCheck, Ar) => {
            "لا يرى \u{2068}Kabar\u{2069} الرسم على هذا الجهاز بعد. انظر إلى رسمتك: هل تشبه الكلمة؟"
        }
        (SelfCheck, En) => {
            "Kabar can't see drawings on this device yet. Look at yours: does it look like the word?"
        }
        (Yes, ArEg) => "آه",
        (Yes, Ar) => "نعم",
        (Yes, En) => "Yes",
        (NotReally, ArEg) => "مش أوي",
        (NotReally, Ar) => "ليس تمامًا",
        (NotReally, En) => "Not really",
        (NextRound, ArEg) => "كلمة تانية",
        (NextRound, Ar) => "كلمة أخرى",
        (NextRound, En) => "Next word",
        (Score, ArEg) => "عرفهم \u{2068}Kabar\u{2069}",
        (Score, Ar) => "خمّنها \u{2068}Kabar\u{2069}",
        (Score, En) => "guessed",

        (MapIntro, ArEg) => "اكتب موضوع، و \u{2068}Kabar\u{2069} يعمله خريطة كلمات تكمّل عليها بالقلم.",
        (MapIntro, Ar) => "اكتب موضوعًا، وسيصنع \u{2068}Kabar\u{2069} خريطة كلمات تكملها بالقلم.",
        (MapIntro, En) => "Name a topic and Kabar draws a word map you can extend with the pen.",
        (Topic, ArEg | Ar) => "di pasar، keluarga، cuaca…",
        (Topic, En) => "di pasar, keluarga, cuaca…",
        (Make, ArEg) => "اعمل الخريطة",
        (Make, Ar) => "أنشئ الخريطة",
        (Make, En) => "Make the map",
        (Making, ArEg) => "بعمل الخريطة…",
        (Making, Ar) => "جارٍ إنشاء الخريطة…",
        (Making, En) => "Making the map…",
        (Unverified, ArEg) => "لسه متراجعتش",
        (Unverified, Ar) => "غير مُراجَع",
        (Unverified, En) => "unverified",
        (Maps, ArEg) => "خرايطك",
        (Maps, Ar) => "خرائطك",
        (Maps, En) => "Your maps",

        (ListenIntro, ArEg) => {
            "حطه جنب يوتيوب في الشاشة المقسومة. سمعت كلمة؟ اكتبها زي ما سمعتها، و \u{2068}Kabar\u{2069} يقولك ممكن تكون إيه."
        }
        (ListenIntro, Ar) => {
            "ضعه بجانب يوتيوب في الشاشة المقسومة. سمعت كلمة؟ اكتبها كما سمعتها، وسيقترح \u{2068}Kabar\u{2069} ما قد تكون."
        }
        (ListenIntro, En) => {
            "Put this next to YouTube in split screen. Heard a word? Write it the way it sounded and Kabar suggests what it could be."
        }
        (HeardIt, ArEg) => "اللي سمعته",
        (HeardIt, Ar) => "ما سمعته",
        (HeardIt, En) => "What you heard",
        (NoSuggestions, ArEg) => "مش لاقي كلمة قريبة. ممكن تعلّمها لـ \u{2068}Kabar\u{2069}.",
        (NoSuggestions, Ar) => "لا كلمة قريبة. يمكنك تعليمها لـ \u{2068}Kabar\u{2069}.",
        (NoSuggestions, En) => "No close word yet. You could teach it to Kabar.",
        (AddReview, ArEg) => "للمراجعة",
        (AddReview, Ar) => "أضف للمراجعة",
        (AddReview, En) => "Add to review",
        (TeachIt, ArEg) => "علّمها لـ \u{2068}Kabar\u{2069}",
        (TeachIt, Ar) => "علّمها لـ \u{2068}Kabar\u{2069}",
        (TeachIt, En) => "Teach Kabar",
        (Saved, ArEg) => "اتحفظت",
        (Saved, Ar) => "حُفظت",
        (Saved, En) => "Saved",
        (ScribbleHere, ArEg) => "اخربش هنا بالقلم",
        (ScribbleHere, Ar) => "اكتب هنا بالقلم",
        (ScribbleHere, En) => "Scribble here",

        (WriteTab, ArEg) => "إملا",
        (WriteTab, Ar) => "إملاء",
        (WriteTab, En) => "Dictation",
        (DictationIntro, ArEg) => {
            "اسمع الكلمة واكتبها بالقلم. \u{2068}Kabar\u{2069} هيعلّم على الحروف اللي محتاجة تتظبط."
        }
        (DictationIntro, Ar) => {
            "استمع إلى الكلمة واكتبها بالقلم، وسيضع \u{2068}Kabar\u{2069} علامة على الحروف التي تحتاج تصحيحًا."
        }
        (DictationIntro, En) => {
            "Listen to the word and write it with the pen; Kabar marks the letters to fix."
        }
        (PlayAgain, ArEg) => "اسمعها تاني",
        (PlayAgain, Ar) => "استمع مجددًا",
        (PlayAgain, En) => "Play again",
        (WriteHere, ArEg) => "اكتب إجابتك هنا بالقلم",
        (WriteHere, Ar) => "اكتب إجابتك هنا بالقلم",
        (WriteHere, En) => "Write your answer here",
        (Check, ArEg) => "صحّح",
        (Check, Ar) => "تحقق",
        (Check, En) => "Check",
        (Clear, ArEg) => "امسح",
        (Clear, Ar) => "امسح",
        (Clear, En) => "Clear",
        (CorrectForm, ArEg) => "الصح:",
        (CorrectForm, Ar) => "الصواب:",
        (CorrectForm, En) => "Correct:",
        (Aksara, ArEg | Ar) => "أكشارا جاوة",
        (Aksara, En) => "Aksara Jawa",
        (AksaraIntro, ArEg) => {
            "اتفرّج على ترتيب الخطوط، اكتب فوق الحرف الباهت، وبعدين اكتبه لوحدك. الفرشاة بتتقل وتخف مع ضغط القلم."
        }
        (AksaraIntro, Ar) => {
            "شاهد ترتيب الخطوط، ثم اكتب فوق الحرف الباهت، ثم اكتبه وحدك. تتغير سماكة الفرشاة مع ضغط القلم."
        }
        (AksaraIntro, En) => {
            "Watch the stroke order, trace over the faint letter, then write it on your own. The brush follows your pen pressure."
        }
        (Letters, ArEg | Ar) => "الحروف",
        (Letters, En) => "Letters",
        (Signs, ArEg) => "علامات الحركات",
        (Signs, Ar) => "علامات الحركات",
        (Signs, En) => "Vowel signs",
        (Finals, ArEg | Ar) => "العلامات الأخيرة",
        (Finals, En) => "Final signs",
        (Words, ArEg | Ar) => "كلمات",
        (Words, En) => "Words",
        (Watch, ArEg) => "اتفرّج",
        (Watch, Ar) => "شاهد",
        (Watch, En) => "Watch",
        (Trace, ArEg) => "اكتب فوقه",
        (Trace, Ar) => "شفّ الحرف",
        (Trace, En) => "Trace",
        (WriteFree, ArEg) => "اكتب لوحدك",
        (WriteFree, Ar) => "اكتب وحدك",
        (WriteFree, En) => "Write",
        (Replay, ArEg | Ar) => "أعِد",
        (Replay, En) => "Replay",
        (OrderUnverified, ArEg) => "ترتيب الخطوط ده مسودة لسه محدش راجعها",
        (OrderUnverified, Ar) => "ترتيب الخطوط مسودة لم تُراجَع بعد",
        (OrderUnverified, En) => "Stroke order is a draft, not reviewed yet",
        (CheckShape, ArEg) => "شوف الشكل",
        (CheckShape, Ar) => "افحص الشكل",
        (CheckShape, En) => "Check shape",
        (AskTeacher, ArEg) => "اسأل المدرّس",
        (AskTeacher, Ar) => "اسأل المعلّم",
        (AskTeacher, En) => "Ask the teacher",
        (TeacherSays, ArEg) => "المدرّس بيقول:",
        (TeacherSays, Ar) => "يقول المعلّم:",
        (TeacherSays, En) => "The teacher says:",
        (TraceDone, ArEg) => "حلو أوي! الشكل مظبوط. جرّب تكتبه لوحدك دلوقتي.",
        (TraceDone, Ar) => "أحسنت! الشكل صحيح. جرّب أن تكتبه وحدك الآن.",
        (TraceDone, En) => "Lovely, the shape is right. Now try it on your own.",
        (WriteDone, ArEg) => "برافو! كتبته لوحدك وشكله سليم.",
        (WriteDone, Ar) => "أحسنت! كتبته وحدك وشكله سليم.",
        (WriteDone, En) => "Well done, you wrote it on your own and the shape is sound.",
        (TryWriting, ArEg) => "اكتبه لوحدك",
        (TryWriting, Ar) => "اكتبه وحدك",
        (TryWriting, En) => "Write it alone",
        (NextLetter, ArEg | Ar) => "اللي بعده",
        (NextLetter, En) => "Next one",
        (TryAgainHere, ArEg) => "قربت! امسح وجرّب تاني، وخلي بالك من النقطة دي:",
        (TryAgainHere, Ar) => "اقتربت! امسح وحاول مرة أخرى، وانتبه إلى هذا:",
        (TryAgainHere, En) => "Close! Clear it and try once more, watching this:",
        (NothingYet, ArEg) => "لسه مفيش حاجة مكتوبة.",
        (NothingYet, Ar) => "لم يُكتب شيء بعد.",
        (NothingYet, En) => "Nothing written yet.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_has_three_languages() {
        // The match is exhaustive per (msg, lang); this keeps empty strings out.
        for l in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
            for m in [T::Notebook, T::NotConnected, T::SelfCheck, T::ListenIntro, T::CorrectForm] {
                assert!(!tr(l, m).trim().is_empty());
            }
        }
    }
}
