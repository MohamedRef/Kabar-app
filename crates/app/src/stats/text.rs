//! Stats and level-check strings in Egyptian Arabic, MSA and English. Same
//! typed approach as `crate::i18n` (a missing translation is a compile
//! error), kept next to the screens so their strings live in one place.

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S {
    Title,
    Back,
    OpenStats,
    OpenStatsHint,
    // heatmap
    HeatTitle,
    HeatSummary,
    HeatLess,
    HeatMore,
    HeatCell,
    HeatCellEmpty,
    // words
    WordsTitle,
    Known,
    Learning,
    New,
    KnownRule,
    NoWords,
    LangId,
    LangJv,
    WordsSummary,
    // retention
    RetentionTitle,
    Week,
    Month,
    Mature,
    Young,
    AllReviews,
    NoReviewsYet,
    RetentionRule,
    // time
    TimeTitle,
    StudyDays,
    AllTime,
    // forecast
    ForecastTitle,
    ForecastSummary,
    ForecastEmpty,
    ForecastBar,
    Today,
    In30Days,
    // level check
    CheckTitle,
    CheckIntro,
    CheckNoTest,
    CheckStart,
    CheckSeeResult,
    CheckLater,
    CheckNext,
    CheckClose,
    CheckLast,
    CheckNever,
    CheckDoNow,
    CheckNotDue,
    ResultUp,
    ResultStay,
    ResultDown,
    StayBody,
    DownBody,
    FlightInvite,
    FlightButton,
    CheckScore,
    RadarTitle,
    RadarNow,
    RadarBefore,
    RadarNotMeasured,
    RadarSummary,
    RadarFirst,
    ForecastLevelTitle,
    ForecastNoPace,
    ForecastEstimate,
    Done,
}

pub fn st(lang: UiLanguage, msg: S) -> &'static str {
    use S::*;
    use UiLanguage::*;
    match (msg, lang) {
        (Title, ArEg) => "إحصائياتك",
        (Title, Ar) => "إحصاءاتك",
        (Title, En) => "Your stats",
        (Back, ArEg | Ar) => "رجوع",
        (Back, En) => "Back",
        (OpenStats, ArEg) => "كل الإحصائيات",
        (OpenStats, Ar) => "كل الإحصاءات",
        (OpenStats, En) => "All stats",
        (OpenStatsHint, ArEg) => "أيام المذاكرة، الكلمات اللي بقت ثابتة معاك، والتذكّر.",
        (OpenStatsHint, Ar) => "أيام الدراسة، والكلمات التي رسخت لديك، ونسبة التذكّر.",
        (OpenStatsHint, En) => "Study days, the words that have stuck, and how well you remember.",

        (HeatTitle, ArEg) => "أيام المذاكرة",
        (HeatTitle, Ar) => "أيام الدراسة",
        (HeatTitle, En) => "Study days",
        (HeatSummary, ArEg) => "آخر {weeks} أسبوع: ذاكرت في {days} منهم، والوقت كله {minutes}.",
        (HeatSummary, Ar) => "آخر {weeks} أسبوعًا: عدد أيام الدراسة {days}، بمجموع {minutes}.",
        (HeatSummary, En) => "Last {weeks} weeks: you studied on {days} days, {minutes} in all.",
        (HeatLess, ArEg) => "أقل",
        (HeatLess, Ar) => "أقل",
        (HeatLess, En) => "Less",
        (HeatMore, ArEg) => "أكتر",
        (HeatMore, Ar) => "أكثر",
        (HeatMore, En) => "More",
        (HeatCell, ArEg) => "{date}: {minutes}، مراجعات: {reviews}",
        (HeatCell, Ar) => "{date}: {minutes}، مراجعات: {reviews}",
        (HeatCell, En) => "{date}: {minutes}, reviews: {reviews}",
        (HeatCellEmpty, ArEg) => "{date}: راحة",
        (HeatCellEmpty, Ar) => "{date}: راحة",
        (HeatCellEmpty, En) => "{date}: a rest day",

        (WordsTitle, ArEg) => "الكلمات اللي تعرفها",
        (WordsTitle, Ar) => "الكلمات التي تعرفها",
        (WordsTitle, En) => "Words you know",
        (Known, ArEg) => "ثابتة معاك",
        (Known, Ar) => "راسخة",
        (Known, En) => "Known",
        (Learning, ArEg) => "بتتعلمها",
        (Learning, Ar) => "قيد التعلّم",
        (Learning, En) => "Learning",
        (New, ArEg) => "لسه جديدة",
        (New, Ar) => "جديدة",
        (New, En) => "New",
        (KnownRule, ArEg) => {
            "«ثابتة» يعني غالبًا هتفتكرها حتى بعد تلات أسابيع من آخر مراجعة. «بتتعلمها» يعني راجعتها ولسه بتترسّخ."
        }
        (KnownRule, Ar) => {
            "«راسخة» تعني أنك على الأرجح ستتذكرها حتى بعد ثلاثة أسابيع من آخر مراجعة. «قيد التعلّم» تعني أنك راجعتها وما زالت تترسّخ."
        }
        (KnownRule, En) => {
            "“Known” means you'd very likely still recall it three weeks after your last review. “Learning” means you've reviewed it and it's settling in."
        }
        (NoWords, ArEg) => "لسه مفيش كلمات هنا. أول ما تراجع شوية كلمات هتلاقيها بتتجمّع.",
        (NoWords, Ar) => "لا كلمات هنا بعد. ما إن تراجع بعض الكلمات حتى تبدأ بالظهور.",
        (NoWords, En) => "No words here yet. Review a few and you'll see them gather.",
        (LangId, ArEg | Ar) => "الإندونيسية",
        (LangId, En) => "Indonesian",
        (LangJv, ArEg | Ar) => "الجاوية",
        (LangJv, En) => "Javanese",
        (WordsSummary, ArEg) => "{lang}: {known} ثابتة، {learning} بتتعلمها، {new} لسه جديدة.",
        (WordsSummary, Ar) => "{lang}: {known} راسخة، و{learning} قيد التعلّم، و{new} جديدة.",
        (WordsSummary, En) => "{lang}: {known} known, {learning} learning, {new} new.",

        (RetentionTitle, ArEg) => "بتفتكر قد إيه",
        (RetentionTitle, Ar) => "نسبة التذكّر",
        (RetentionTitle, En) => "True retention",
        (Week, ArEg) => "آخر {n} أيام",
        (Week, Ar) => "آخر {n} أيام",
        (Week, En) => "Last {n} days",
        (Month, ArEg) => "آخر {n} يوم",
        (Month, Ar) => "آخر {n} يومًا",
        (Month, En) => "Last {n} days",
        (Mature, ArEg) => "كلمات قديمة",
        (Mature, Ar) => "بطاقات ناضجة",
        (Mature, En) => "Mature cards",
        (Young, ArEg) => "كلمات حديثة",
        (Young, Ar) => "بطاقات حديثة",
        (Young, En) => "Young cards",
        (AllReviews, ArEg) => "الكل",
        (AllReviews, Ar) => "الكل",
        (AllReviews, En) => "All",
        (NoReviewsYet, ArEg) => "لسه مفيش",
        (NoReviewsYet, Ar) => "لا يوجد بعد",
        (NoReviewsYet, En) => "none yet",
        (RetentionRule, ArEg) => {
            "النسبة دي من المراجعات اللي افتكرت فيها كلمة شفتها قبل كده (أي إجابة غير «تاني»). «القديمة» هي اللي كانت مستنياك {days} يوم أو أكتر. حوالي {pct} ده المعدّل الصحي."
        }
        (RetentionRule, Ar) => {
            "النسبة من المراجعات التي تذكّرت فيها كلمة سبق أن رأيتها (أي إجابة غير «مجددًا»). «الناضجة» هي التي انتظرتك {days} يومًا أو أكثر. نحو {pct} هو المعدّل الصحي."
        }
        (RetentionRule, En) => {
            "The share of reviews of words you'd seen before that you remembered (anything but “Again”). “Mature” cards had waited {days} days or more. Around {pct} is the healthy mark."
        }

        (TimeTitle, ArEg) => "وقت المذاكرة",
        (TimeTitle, Ar) => "وقت الدراسة",
        (TimeTitle, En) => "Time studied",
        (StudyDays, ArEg) => "أيام ذاكرت فيها: {n}",
        (StudyDays, Ar) => "أيام الدراسة: {n}",
        (StudyDays, En) => "Study days: {n}",
        (AllTime, ArEg) => "من الأول",
        (AllTime, Ar) => "منذ البداية",
        (AllTime, En) => "All time",

        (ForecastTitle, ArEg) => "المراجعات الجاية",
        (ForecastTitle, Ar) => "المراجعات القادمة",
        (ForecastTitle, En) => "Reviews ahead",
        (ForecastSummary, ArEg) => {
            "في الـ{n} يوم الجايين: {total} مراجعات في المجموع، حوالي {avg} في اليوم. أكتر يوم: {day} ({peak})."
        }
        (ForecastSummary, Ar) => {
            "خلال {n} يومًا القادمة: مجموع المراجعات {total}، نحو {avg} يوميًّا. أكثر الأيام: {day} ({peak})."
        }
        (ForecastSummary, En) => {
            "Next {n} days: {total} reviews, about {avg} a day. Busiest: {day} ({peak})."
        }
        (ForecastEmpty, ArEg) => "مفيش مراجعات متجدولة في الـ{n} يوم الجايين. الكلمات مرتاحة.",
        (ForecastEmpty, Ar) => "لا مراجعات مجدولة خلال {n} يومًا القادمة. الكلمات مستقرة.",
        (ForecastEmpty, En) => "Nothing scheduled for the next {n} days. Your words are resting.",
        (ForecastBar, ArEg | Ar) => "{day}: {n}",
        (ForecastBar, En) => "{day}: {n}",
        (Today, ArEg) => "النهارده",
        (Today, Ar) => "اليوم",
        (Today, En) => "Today",
        (In30Days, ArEg) => "بعد {n} يوم",
        (In30Days, Ar) => "بعد {n} يومًا",
        (In30Days, En) => "In {n} days",

        (CheckTitle, ArEg | Ar) => "فحص المستوى الشهري",
        (CheckTitle, En) => "Monthly level check",
        (CheckIntro, ArEg) => {
            "حوالي عشر دقايق من أسئلة {level}: شوية استماع وقراية وكلمات وقواعد، وسؤال كتابة وكلام. مفيش تصحيح وإنت بتحل، ومش امتحان. في الآخر هتشوف خريطة مهاراتك والتوقّع."
        }
        (CheckIntro, Ar) => {
            "نحو عشر دقائق من أسئلة {level}: قليل من الاستماع والقراءة والمفردات والقواعد، وسؤال كتابة وآخر للمحادثة. لا تصحيح أثناء الحل، وليس امتحانًا. في النهاية ترى خريطة مهاراتك والتوقّع."
        }
        (CheckIntro, En) => {
            "About ten minutes of {level} questions: a little listening, reading, vocabulary and grammar, plus one writing and one speaking task. No marking as you go, and it's not an exam. At the end you'll see your skill map and the forecast."
        }
        (CheckNoTest, ArEg) => {
            "لسه مفيش أسئلة فحص لـ {level} في كبر، فالمرة دي هنبص على مذاكرتك في الشهر اللي فات بس."
        }
        (CheckNoTest, Ar) => {
            "لا توجد بعد أسئلة فحص لـ {level} في كبر، لذا سننظر هذه المرة في دراستك خلال الشهر الماضي فقط."
        }
        (CheckNoTest, En) => {
            "Kabar has no check questions for {level} yet, so this time we'll look at your last month of study only."
        }
        (CheckStart, ArEg) => "يلا نبدأ",
        (CheckStart, Ar) => "لنبدأ",
        (CheckStart, En) => "Let's start",
        (CheckSeeResult, ArEg) => "وريني النتيجة",
        (CheckSeeResult, Ar) => "اعرض النتيجة",
        (CheckSeeResult, En) => "Show me",
        (CheckLater, ArEg) => "بعدين",
        (CheckLater, Ar) => "لاحقًا",
        (CheckLater, En) => "Later",
        (CheckNext, ArEg) => "اللي بعده",
        (CheckNext, Ar) => "التالي",
        (CheckNext, En) => "Next",
        (CheckClose, ArEg) => "اقفل الفحص (مش هيتحسب)",
        (CheckClose, Ar) => "أغلق الفحص (لن يُحتسب)",
        (CheckClose, En) => "Close the check (it won't count)",
        (CheckLast, ArEg) => "آخر فحص: {date}",
        (CheckLast, Ar) => "آخر فحص: {date}",
        (CheckLast, En) => "Last check: {date}",
        (CheckNever, ArEg) => "لسه ماعملتش فحص. أول واحد بعد حوالي شهر من المذاكرة.",
        (CheckNever, Ar) => "لم تُجرِ فحصًا بعد. يأتي الأول بعد نحو شهر من الدراسة.",
        (CheckNever, En) => "No check yet. The first comes after about a month of study.",
        (CheckDoNow, ArEg) => "اعمل الفحص",
        (CheckDoNow, Ar) => "ابدأ الفحص",
        (CheckDoNow, En) => "Take the check",
        (CheckNotDue, ArEg) => "الفحص الجاي حوالي {date}، وتقدر تعمله قبلها لو حابب.",
        (CheckNotDue, Ar) => "الفحص التالي نحو {date}، ويمكنك إجراؤه قبل ذلك إن شئت.",
        (CheckNotDue, En) => "The next check is around {date}; you're welcome to take it sooner.",
        (ResultUp, ArEg) => "طلعت مستوى!",
        (ResultUp, Ar) => "انتقلت إلى مستوى جديد!",
        (ResultUp, En) => "You've moved up!",
        (ResultStay, ArEg) => "ماشي في طريقك",
        (ResultStay, Ar) => "تسير في طريقك",
        (ResultStay, En) => "On your way",
        (ResultDown, ArEg) => "هنثبّت الأساس",
        (ResultDown, Ar) => "سنثبّت الأساس",
        (ResultDown, En) => "Let's make the base rock-solid",
        (StayBody, ArEg) => "هتكمّل في {level}. كل يوم بتذاكر فيه بيبني عليه اللي بعده.",
        (StayBody, Ar) => "ستواصل في {level}. كل يوم تدرس فيه يبني على ما قبله.",
        (StayBody, En) => "You'll carry on in {level}. Every day you study builds on the last.",
        (DownBody, ArEg) => {
            "هنقعد شوية في {level} نقوّي الأساس. كلماتك وقنديلك وكوينك وكل اللي حققته زي ما هو، وطريقك في الرحلة مفتوح زي ما هو."
        }
        (DownBody, Ar) => {
            "سنمكث قليلًا في {level} لتقوية الأساس. كلماتك وقنديلك وعملاتك وكل ما حققته باقٍ كما هو، والرحلة مفتوحة كما كانت."
        }
        (DownBody, En) => {
            "We'll spend a little time in {level} strengthening the base. Your words, Pelita, Koin and everything you've earned stay, and the whole journey stays open."
        }
        (FlightInvite, ArEg) => {
            "نتيجتك في الفحص الصغير وصلت لدرجة النجاح ({mark}). لو حابب تطلع المستوى، جرّب الاختبار الكامل بتاع {level}."
        }
        (FlightInvite, Ar) => {
            "بلغت نتيجتك في الفحص القصير درجة النجاح ({mark}). إن أردت الانتقال، جرّب الاختبار الكامل لـ {level}."
        }
        (FlightInvite, En) => {
            "Your mini check reached the pass mark ({mark}). If you'd like to move up, try the full {level} test."
        }
        (FlightButton, ArEg) => "اركب الطيارة",
        (FlightButton, Ar) => "استقل الطائرة",
        (FlightButton, En) => "Take a flight",
        (CheckScore, ArEg) => "نتيجة الفحص",
        (CheckScore, Ar) => "نتيجة الفحص",
        (CheckScore, En) => "Check score",
        (RadarTitle, ArEg | Ar) => "خريطة مهاراتك",
        (RadarTitle, En) => "Your skill map",
        (RadarNow, ArEg) => "الشهر ده",
        (RadarNow, Ar) => "هذا الشهر",
        (RadarNow, En) => "This month",
        (RadarBefore, ArEg) => "الشهر اللي فات",
        (RadarBefore, Ar) => "الشهر الماضي",
        (RadarBefore, En) => "Last month",
        (RadarNotMeasured, ArEg) => "لسه متقاسش",
        (RadarNotMeasured, Ar) => "لم يُقَس بعد",
        (RadarNotMeasured, En) => "not measured yet",
        (RadarSummary, ArEg | Ar) => "{skill}: {value}",
        (RadarSummary, En) => "{skill}: {value}",
        (RadarFirst, ArEg) => "ده أول فحص، فالمقارنة هتبدأ من الشهر الجاي.",
        (RadarFirst, Ar) => "هذا أول فحص، فتبدأ المقارنة من الشهر القادم.",
        (RadarFirst, En) => "This is your first check, so comparisons start next month.",
        (ForecastLevelTitle, ArEg) => "هتوصل إمتى؟",
        (ForecastLevelTitle, Ar) => "متى تصل؟",
        (ForecastLevelTitle, En) => "When you'll get there",
        (ForecastNoPace, ArEg) => "لما نذاكر كام يوم كمان هقدر أقولك تقريبًا هتوصل إمتى.",
        (ForecastNoPace, Ar) => "بعد بضعة أيام أخرى من الدراسة أستطيع أن أقدّر متى تصل.",
        (ForecastNoPace, En) => "A few more days of study and I can estimate when you'll get there.",
        (ForecastEstimate, ArEg) => "على أساس متوسط {minutes} في اليوم في آخر {days} يوم.",
        (ForecastEstimate, Ar) => "بناءً على متوسط {minutes} يوميًّا خلال آخر {days} يومًا.",
        (ForecastEstimate, En) => "Based on {minutes} a day on average over the last {days} days.",
        (Done, ArEg) => "تمام",
        (Done, Ar) => "تم",
        (Done, En) => "Done",
    }
}
