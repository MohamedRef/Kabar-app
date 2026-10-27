//! Sinetron and Kabar Radio strings in Egyptian Arabic, MSA and English,
//! typed like `crate::i18n` (a missing translation is a compile error).

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    ListenIntro,
    SinetronIntro,
    RadioIntro,
    Episode,
    OpensAfter,
    Watched,
    Listened,
    Play,
    Pause,
    Slower,
    Translations,
    Note,
    TapLine,
    Download,
    Downloading,
    Downloaded,
    RemoveDownload,
    DownloadsUse,
    FreeUpSpace,
    NoDownloads,
    NeedCloud,
    Offline,
    Quota,
    ServerTrouble,
    SystemVoice,
    JavaneseLater,
    Questions,
    NextTime,
    Words,
    Back,
    Broken,
    Scene,
    Intro,
    Dialogue,
    WordSpotlight,
    Recap,
    Outro,
}

pub fn tt(lang: UiLanguage, msg: T) -> &'static str {
    use T::*;
    use UiLanguage::*;
    match (msg, lang) {
        (SinetronIntro, ArEg) => "حلقة جديدة بتفتح مع كل وحدة بتخلّصها في الرحلة.",
        (SinetronIntro, Ar) => "تُفتح حلقة جديدة مع كل وحدة تُنهيها في الرحلة.",
        (SinetronIntro, En) => "A new episode opens with every unit you finish on the Journey.",
        (ListenIntro, ArEg) => "مسلسل قصير بحلقة لكل وحدة بتخلّصها، وراديو بيراجع معاك كلمات الوحدة.",
        (ListenIntro, Ar) => "مسلسل قصير بحلقة لكل وحدة تُنهيها، وإذاعة تراجع معك كلمات الوحدة.",
        (ListenIntro, En) => {
            "A short drama with an episode for every unit you finish, and a radio show that goes over the unit's words."
        }
        (RadioIntro, ArEg) => {
            "حلقة لكل وحدة: Kak Tiara وKak Reza بيراجعوا كلماتها معاك. نزّلها واسمعها في الطريق."
        }
        (RadioIntro, Ar) => {
            "حلقة لكل وحدة: Kak Tiara وKak Reza يراجعان كلماتها معك. نزّلها واستمع إليها في الطريق."
        }
        (RadioIntro, En) => {
            "One episode per unit: Kak Tiara and Kak Reza go over its words with you. Download one for the road."
        }
        (Episode, ArEg | Ar) => "الحلقة",
        (Episode, En) => "Episode",
        (OpensAfter, ArEg) => "بتفتح بعد وحدة",
        (OpensAfter, Ar) => "تُفتح بعد وحدة",
        (OpensAfter, En) => "Opens after the unit",
        (Watched, ArEg) => "اتفرجت عليها",
        (Watched, Ar) => "شاهدتها",
        (Watched, En) => "Watched",
        (Listened, ArEg) => "سمعتها",
        (Listened, Ar) => "استمعت إليها",
        (Listened, En) => "Listened",
        (Play, ArEg) => "شغّل",
        (Play, Ar) => "تشغيل",
        (Play, En) => "Play",
        (Pause, ArEg) => "وقّف",
        (Pause, Ar) => "إيقاف مؤقت",
        (Pause, En) => "Pause",
        (Slower, ArEg) => "أبطأ شوية",
        (Slower, Ar) => "أبطأ قليلًا",
        (Slower, En) => "A bit slower",
        (Translations, ArEg) => "الترجمة",
        (Translations, Ar) => "الترجمة",
        (Translations, En) => "Translations",
        (Note, ArEg) => "معلومة",
        (Note, Ar) => "ملاحظة",
        (Note, En) => "Note",
        (TapLine, ArEg) => "دوس على أي سطر عشان تسمعه، أو على المعنى عشان تشوفه.",
        (TapLine, Ar) => "اضغط على أي سطر لتسمعه، أو على المعنى لتراه.",
        (TapLine, En) => "Tap any line to hear it, or its meaning to see it.",
        (Download, ArEg) => "نزّلها للأوفلاين",
        (Download, Ar) => "نزّلها للاستماع دون اتصال",
        (Download, En) => "Download for offline",
        (Downloading, ArEg) => "بتنزل…",
        (Downloading, Ar) => "جارٍ التنزيل…",
        (Downloading, En) => "Downloading…",
        (Downloaded, ArEg) => "متنزّلة: بتشتغل من غير نت",
        (Downloaded, Ar) => "منزّلة: تعمل دون اتصال",
        (Downloaded, En) => "Downloaded: plays offline",
        (RemoveDownload, ArEg) => "امسح التنزيل",
        (RemoveDownload, Ar) => "احذف التنزيل",
        (RemoveDownload, En) => "Remove download",
        (DownloadsUse, ArEg) => "الحلقات المتنزّلة واخدة",
        (DownloadsUse, Ar) => "تشغل الحلقات المنزّلة",
        (DownloadsUse, En) => "Downloaded episodes use",
        (FreeUpSpace, ArEg) => "فضّي المساحة",
        (FreeUpSpace, Ar) => "أفرغ المساحة",
        (FreeUpSpace, En) => "Free up space",
        (NoDownloads, ArEg) => "مفيش حلقات متنزّلة لسه.",
        (NoDownloads, Ar) => "لا توجد حلقات منزّلة بعد.",
        (NoDownloads, En) => "No downloaded episodes yet.",
        (NeedCloud, ArEg) => "سجّل دخولك في Kabar Cloud عشان تسمع الأصوات وتنزّل الحلقات.",
        (NeedCloud, Ar) => "سجّل الدخول إلى Kabar Cloud لتسمع الأصوات وتنزّل الحلقات.",
        (NeedCloud, En) => "Sign in to Kabar Cloud to hear the voices and download episodes.",
        (Offline, ArEg) => "إنت أوفلاين دلوقتي. المتنزّل بيشتغل عادي، والتنزيل بيكمّل من مكانه لما النت يرجع.",
        (Offline, Ar) => {
            "أنت غير متصل الآن. ما نُزّل يعمل كالمعتاد، ويُستأنف التنزيل من حيث توقف عند عودة الاتصال."
        }
        (Offline, En) => {
            "You're offline right now. Downloaded episodes play as usual, and downloads pick up where they stopped."
        }
        (Quota, ArEg) => "رصيد الأصوات للشهر ده خلص. المتنزّل شغال، والباقي يرجع أول الشهر الجاي.",
        (Quota, Ar) => "نفد رصيد الأصوات لهذا الشهر. ما نُزّل يعمل، والباقي يعود مطلع الشهر القادم.",
        (Quota, En) => {
            "This month's voice allowance is used up. Downloaded episodes still play; the rest is back next month."
        }
        (ServerTrouble, ArEg) => "Kabar Cloud مش بيرد دلوقتي. جرّب كمان شوية.",
        (ServerTrouble, Ar) => "لا يستجيب Kabar Cloud الآن. حاول بعد قليل.",
        (ServerTrouble, En) => "Kabar Cloud isn't answering right now. Try again in a little while.",
        (SystemVoice, ArEg) => "بنقرا بصوت الجهاز لحد ما أصوات كبر ترجع.",
        (SystemVoice, Ar) => "نقرأ بصوت الجهاز إلى أن تعود أصوات كبر.",
        (SystemVoice, En) => "Reading with the device's voice until Kabar's voices are back.",
        (JavaneseLater, ArEg) => "الصوت الجاوي جاي بعدين؛ اقرا مع النص دلوقتي.",
        (JavaneseLater, Ar) => "الصوت الجاوي لاحقًا؛ اقرأ النص الآن.",
        (JavaneseLater, En) => "Javanese audio comes later; read along for now.",
        (Questions, ArEg) => "يلا على الأسئلة",
        (Questions, Ar) => "إلى الأسئلة",
        (Questions, En) => "On to the questions",
        (NextTime, ArEg) => "المرة الجاية",
        (NextTime, Ar) => "في الحلقة القادمة",
        (NextTime, En) => "Next time",
        (Words, ArEg | Ar) => "كلمات الحلقة",
        (Words, En) => "Words in this episode",
        (Back, ArEg | Ar) => "رجوع",
        (Back, En) => "Back",
        (Broken, ArEg) => "الحلقة دي فيها مشكلة في الملف، ومش هتشتغل لحد ما تتصلّح.",
        (Broken, Ar) => "في ملف هذه الحلقة مشكلة، ولن تعمل حتى تُصلَح.",
        (Broken, En) => "This episode's file has a problem, so it can't play until it's fixed.",
        (Scene, ArEg | Ar) => "المشهد",
        (Scene, En) => "Scene",
        (Intro, ArEg) => "البداية",
        (Intro, Ar) => "المقدمة",
        (Intro, En) => "Intro",
        (Dialogue, ArEg | Ar) => "حوار",
        (Dialogue, En) => "Dialogue",
        (WordSpotlight, ArEg) => "كلمات في الضوء",
        (WordSpotlight, Ar) => "كلمات تحت الضوء",
        (WordSpotlight, En) => "Word spotlight",
        (Recap, ArEg) => "نراجع",
        (Recap, Ar) => "خلاصة",
        (Recap, En) => "Recap",
        (Outro, ArEg | Ar) => "الختام",
        (Outro, En) => "Outro",
    }
}
