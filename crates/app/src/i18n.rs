//! UI text, in Egyptian Arabic. One module per feature: a feature chunk adds
//! its own `pub mod <feature>` block and touches nothing else here.
//! Medical terms stay in English (wrap them in `bdi` in the markup).

pub mod app {
    pub const NAME: &str = "باطنة";
    pub const TAGLINE: &str = "مذاكرة الباطنة، جلسة قصيرة كل مرة";
}

pub mod nav {
    pub const TODAY: &str = "النهارده";
    pub const STUDY: &str = "مذاكرة";
    pub const BANK: &str = "الأسئلة";
    pub const CASES: &str = "حالات";
    pub const ORAL: &str = "شفوي";
    pub const STATS: &str = "تقدّمي";
    pub const SETTINGS: &str = "الإعدادات";
}

pub mod today {
    pub const LIBRARY: &str = "محاضرات الفصل الأول";
    pub const CHAPTER: &str = "Hepato-Biliary Pancreatic Unit";
    pub const READY: &str = "جاهزة";
    pub const WRITING: &str = "بتتكتب";
    pub const ITEMS: &str = "سؤال وكارت";
    pub const PAGES: &str = "صفحات";
    pub const UNVERIFIED_NOTE: &str = "كل المحتوى لسه ما اتراجعش. أول ما تشوف أي سؤال، علّم عليه صح أو غلط.";
}

pub mod study {
    pub const TITLE: &str = "جلسة مذاكرة";
    pub const SETUP_INTRO: &str =
        "اختار المحاضرات والوقت. هنبدأ بالمراجعات اللي عليها الدور، وبعدها الجديد موضوع موضوع.";
    pub const EMPTY_LIBRARY: &str = "لسه مفيش محاضرة جاهزة للمذاكرة هنا. أول ما محاضرة تتكتب هتلاقيها في القايمة، ولحد ده ما يحصل تقدر تذاكر من Anki.";
    pub const LECTURES: &str = "المحاضرات";
    pub const PICK_ALL: &str = "اختار الكل";
    pub const CLEAR_ALL: &str = "امسح الاختيار";
    pub const TOPICS: &str = "المواضيع";
    pub const TOPICS_HINT: &str = "سيبها فاضية عشان تذاكر المحاضرة كلها.";
    pub const TIME: &str = "الوقت";
    pub const MINUTES: &str = "دقيقة";
    pub const HOURS: &str = "ساعة";
    pub const DAYS: &str = "أيام";
    pub const ITEMS: &str = "سؤال";
    pub const REVIEWS: &str = "مراجعة";
    pub const NEW: &str = "جديد";
    pub const ABOUT: &str = "حوالي";
    pub const START: &str = "يلا نبدأ";
    pub const NOTHING_NOW: &str =
        "مفيش حاجة عليها الدور في اللي اخترته دلوقتي. جرّب محاضرة تانية أو ارجع بعدين.";
    pub const END: &str = "إنهاء الجلسة";
    pub const OF: &str = "من";

    pub const BADGE_NEW: &str = "جديد";
    pub const BADGE_REVIEW: &str = "مراجعة";
    pub const BADGE_RETRY: &str = "تاني مرة";
    pub const UNVERIFIED: &str = "لسه ما اتراجعش";
    pub const UNVERIFIED_HINT: &str = "اتكتب من الكتاب ولسه إنت ما أكدتوش.";
    pub const KIND_CARD: &str = "كارت";
    pub const KIND_CLOZE: &str = "كمّل الفراغ";
    pub const KIND_ENUMERATE: &str = "عدّد";
    pub const KIND_ESSAY: &str = "مقالي";
    pub const KIND_MCQ: &str = "اختيار";
    pub const KIND_CASE: &str = "حالة";
    pub const KIND_TABLE: &str = "جدول";
    pub const KIND_ORDER: &str = "رتّب";
    pub const KIND_CALC: &str = "حساب";
    pub const BOOK_PAGE: &str = "الكتاب ص";
    pub const BOOK_QUOTE: &str = "من الكتاب";
    pub const UPDATE_TITLE: &str = "تحديث عن الكتاب";
    pub const UPDATE_AFTER: &str = "افتحه بعد ما تجاوب";
    pub const UPDATE_BOOK_WINS: &str = "في الامتحان، إجابة الكتاب هي اللي بتتحسب.";
    pub const NOT_HERE_YET: &str = "النوع ده من الأسئلة ليه شاشة لسه بتتبني.";
    pub const SKIP: &str = "عدّي";

    pub const THINK_FIRST: &str = "جاوب في دماغك الأول، وبعدين اكشف.";
    pub const REVEAL: &str = "اكشف الإجابة";
    pub const BLANK: &str = "الفراغ";
    pub const CHECK: &str = "صحّح";
    pub const REVEAL_BLANKS: &str = "مش فاكر، وريني";
    pub const CLOZE_TIP: &str = "الأرقام بتتصحح من غير الوحدات والمسافات.";
    pub const CLOZE_RIGHT: &str = "مظبوط.";
    pub const CLOZE_CLOSE: &str = "قريب جدًا. بص على الإجابة كاملة.";
    pub const CLOZE_WRONG: &str = "الإجابة الصح مكتوبة فوق. هترجعلك تاني قريب.";

    pub const ENUM_GOAL_A: &str = "فيه";
    pub const ENUM_GOAL_B: &str = "نقط، ولو افتكرت";
    pub const ENUM_GOAL_C: &str = "منهم تعدّي.";
    pub const ENUM_PLACEHOLDER: &str = "اكتب اللي فاكره، أو قوله لنفسك بصوت عالي (اختياري)";
    pub const SHOW_POINTS: &str = "خلصت، وريني النقط";
    pub const TICK_WHAT_YOU_GOT: &str = "علّم على اللي افتكرته.";
    pub const PRE_TICKED: &str = "علّمت لك على اللي لقيته في كلامك، وغيّر براحتك.";
    pub const ENUM_PASS: &str = "عدّيت.";
    pub const ENUM_SHORT_A: &str = "فاضلك";
    pub const ENUM_SHORT_B: &str = "عشان تعدّي.";
    pub const WHAT_I_WROTE: &str = "اللي كتبته";

    pub const ESSAY_HOW: &str = "اكتب العناوين بس، كل عنوان في سطر، زي ما هتكتبهم في الامتحان.";
    pub const ESSAY_PLACEHOLDER: &str = "Definition\nPathogenesis\nClinical picture\n…";
    pub const COMPARE: &str = "قارن بالإجابة النموذجية";
    pub const MODEL_OUTLINE: &str = "الإجابة النموذجية";
    pub const TICK_HEADINGS: &str = "علّم على العناوين اللي جت في بالك.";
    pub const HEADINGS_GOT: &str = "العناوين اللي جبتها:";

    pub const GRADE_LABEL: &str = "قيّم نفسك";
    pub const SUGGESTED: &str = "اقتراحي متعلّم عليه، والقرار ليك.";
    pub const AGAIN: &str = "تاني";
    pub const HARD: &str = "صعب";
    pub const GOOD: &str = "تمام";
    pub const EASY: &str = "سهل";
    pub const UNIT_MIN: &str = "د";
    pub const UNIT_HOUR: &str = "س";
    pub const UNIT_DAY: &str = "ي";
    pub const UNIT_MONTH: &str = "ش";

    pub const SUMMARY_TITLE: &str = "خلصت الجلسة، برافو عليك";
    pub const SUMMARY_EMPTY: &str = "الجلسة خلصت";
    pub const SUM_ITEMS: &str = "سؤال";
    pub const SUM_RIGHT: &str = "صح من أول مرة";
    pub const SUM_NEW: &str = "جديد";
    pub const SUM_ALL_SOLID: &str = "كله كان ثابت. المراجعة الجاية هتيجي بعد فترة أطول.";
    pub const SUM_BACK_SOON: &str = "هيرجعولك قريب";
    pub const SUM_BACK_SOON_HINT: &str = "دول عادي يتلخبطوا أول مرة. هيتكرروا لحد ما يثبتوا.";
    pub const NEXT_SESSION: &str = "الجلسة الجاية:";
    pub const NEXT_TOMORROW: &str = "ارجع بكرة لجلسة قصيرة، وهتلاقي المراجعات مستنياك.";
    pub const ANOTHER: &str = "جلسة كمان";
    pub const NOW: &str = "دلوقتي";
    pub const AFTER: &str = "بعد";
    pub const TOMORROW: &str = "بكرة";
}

pub mod soon {
    pub const TITLE: &str = "الشاشة دي بتتبني دلوقتي";
    pub const BODY: &str = "هتظهر هنا أول ما تخلص. لحد ما ده يحصل، تقدر تذاكر نفس المحتوى من Anki.";
}

pub mod table {
    pub const HINT_TAP: &str = "دوس على كل ؟ تشوف الإجابة، وبعدين علّم: عرفتها ✓ ولا لأ ✗.";
    pub const HINT_TYPE: &str = "اكتب كل خانة فاضية، وبعدين دوس صحّح.";
    pub const REVEAL_CELL: &str = "اكشف الخانة";
    pub const TYPE_CELL: &str = "اكتب الخانة";
    pub const REVEAL_ALL: &str = "اكشف الكل";
    pub const CHECK_TYPED: &str = "صحّح";
    pub const TYPING_ON: &str = "عايز أكتب";
    pub const TYPING_OFF: &str = "من غير كتابة";
    pub const KNEW_IT: &str = "عرفتها";
    pub const MISSED_IT: &str = "ماعرفتهاش";
    pub const MARK_RIGHT: &str = "اعتبرها صح";
    pub const MARK_WRONG: &str = "اعتبرها غلط";

    pub fn mark_each(left: usize) -> String {
        if left == 1 {
            "فاضل خانة واحدة: عرفتها ولا لأ؟".to_string()
        } else {
            format!("فاضل {left} خانات: علّم على كل واحدة عرفتها ولا لأ.")
        }
    }

    pub fn recalled(right: usize, total: usize) -> String {
        if right == total {
            format!("كل الخانات صح ({total} من {total}). حلو أوي!")
        } else {
            format!("افتكرت {right} من {total} خانات.")
        }
    }
}

pub mod order {
    pub const HINT: &str =
        "دوس على الخطوات بالترتيب. تقدر ترجّع خطوة بالدوس عليها، أو تحركها بالأسهم أو بالسحب.";
    pub const PLACED: &str = "ترتيبك";
    pub const YOUR_ORDER: &str = "ترتيبك";
    pub const POOL: &str = "الخطوات";
    pub const NEXT_SLOT: &str = "الخطوة الجاية هنا";
    pub const CORRECT_ORDER: &str = "الترتيب في الكتاب";
    pub const CHECK: &str = "صحّح";
    pub const CLEAR: &str = "ابدأ من الأول";
    pub const SEND_BACK: &str = "رجّعها للخطوات";
    pub const MOVE_UP: &str = "حركها لفوق";
    pub const MOVE_DOWN: &str = "حركها لتحت";
    pub const IN_PLACE: &str = "في مكانها";
    pub const ALL_IN_PLACE: &str = "كل الخطوات في مكانها. ترتيب ممتاز!";

    pub fn in_place(right: usize, total: usize) -> String {
        format!("{right} من {total} في مكانها. الخطوات المعلّمة بالأحمر محتاجة نظرة تانية.")
    }

    pub fn should_be(position: usize) -> String {
        format!("مكانها {position}")
    }
}

pub mod oral {
    pub const TITLE: &str = "امتحان شفوي";
    pub const INTRO: &str = "التطبيق يسألك بصوت عالي، وانت تجاوب بصوتك. بعد وقت التفكير يقولك الإجابة، وانت تقيّم نفسك. ينفع وانت ماشي.";
    pub const LECTURE: &str = "المحاضرة";
    pub const ALL_LECTURES: &str = "كل المحاضرات الجاهزة";
    pub const SIZE: &str = "عدد الأسئلة";
    pub const THINK: &str = "وقت التفكير";
    pub const SECONDS: &str = "ثانية";
    pub const VOICE: &str = "اقرا الأسئلة بصوت عالي";
    pub const RATE: &str = "سرعة الصوت";
    pub const RATES: [&str; 3] = ["أبطأ", "عادي", "أسرع"];
    pub const AUTO: &str = "كمّل لوحدك بعد الإجابة";
    pub const AUTO_HINT: &str =
        "لو ما ضغطتش على تقييم في ٨ ثواني، يعدّي للسؤال اللي بعده من غير ما يسجّل حاجة.";
    pub const AVAILABLE: &str = "سؤال ينفع يتسأل شفوي";
    pub const START: &str = "ابدأ";
    pub const EMPTY: &str = "لسه مفيش محاضرات جاهزة فيها أسئلة شفوي. أول ما محاضرة تخلص هتلاقيها هنا.";
    pub const NO_VOICE: &str = "مفيش صوت إنجليزي شغّال على الجهاز ده، فالأسئلة هتظهر مكتوبة بس. على أندرويد: الإعدادات ← تحويل النص إلى كلام ← نزّل صوت English.";

    pub const LISTEN: &str = "اسمع السؤال";
    pub const THINKING: &str = "فكّر وجاوب بصوتك";
    pub const WHAT_ELSE: &str = "وإيه كمان؟";
    pub const ANSWER: &str = "الإجابة";
    pub const FOLLOW_UP: &str = "سؤال الممتحن بعد الكارت";
    pub const AGAIN_BADGE: &str = "تاني مرة";
    pub const PASS: &str = "كفاية تذكر";
    pub const OF: &str = "من";
    pub const PAGE: &str = "صفحة";
    pub const LEFT: &str = "فاضل";
    pub const SHOW: &str = "اعرض الإجابة";
    pub const REPEAT: &str = "عيد السؤال";
    pub const AGAIN: &str = "تاني";
    pub const AGAIN_HINT: &str = "هيرجعلك بعد ٣ أسئلة";
    pub const GOOD: &str = "تمام";
    pub const GOOD_HINT: &str = "جاوبت صح";
    pub const END: &str = "إنهاء";

    pub const DONE: &str = "خلصت الجلسة";
    pub const DONE_GOOD: &str = "تمام";
    pub const DONE_AGAIN: &str = "تاني";
    pub const DONE_HEARD: &str = "سمعتها بس";
    pub const MISSED: &str = "الأسئلة اللي محتاجة مراجعة";
    pub const NONE_MISSED: &str = "ولا سؤال وقع منك. شغل حلو!";
    pub const ANOTHER: &str = "جلسة كمان";
    pub const BACK: &str = "رجوع للإعدادات";
}

pub mod case {
    pub const TAB_CASES: &str = "حالات إكلينيكية";
    pub const TAB_CALC_ITEMS: &str = "تمارين بالأرقام";
    pub const TAB_CALCULATORS: &str = "الحاسبات";
    pub const INTRO: &str = "كل حالة بتكشف بيانات جديدة خطوة بخطوة، وفي كل خطوة قرار واحد.";
    pub const EMPTY: &str = "لسه مفيش حالات. هتظهر هنا أول ما محاضراتها تتكتب، ولحد ده الحاسبات شغالة.";
    pub const EMPTY_CALC: &str = "لسه مفيش تمارين بالأرقام. هتظهر هنا مع المحاضرات، ولحد ده جرّب الحاسبات.";
    pub const STEPS: &str = "خطوات";
    pub const STEP: &str = "خطوة";
    pub const OF: &str = "من";
    pub const NEW_DATA: &str = "بيانات جديدة";
    pub const RIGHT: &str = "صح، برافو.";
    pub const WRONG: &str = "مش دي. الإجابة الصح متعلّمة.";
    pub const SHOW_ANSWER: &str = "ورّيني الإجابة";
    pub const GOT_IT: &str = "جاوبتها صح";
    pub const MISSED: &str = "فاتتني";
    pub const NEXT: &str = "الخطوة اللي بعدها";
    pub const FINISH: &str = "خلّص الحالة";
    pub const PAGE: &str = "الكتاب صفحة";
    pub const SCORE: &str = "خطوات صح";
    pub const ALL_RIGHT: &str = "كل الخطوات صح. الحالة دي هترجعلك بعد مدة أطول.";
    pub const SOME_RIGHT: &str = "حلو. بص على الخطوات اللي فاتتك، والحالة هترجعلك قريب في المراجعة.";
    pub const FEW_RIGHT: &str = "ولا يهمك، الحالات دي بتتعلم بالتكرار. هترجعلك بكرة.";
    pub const AGAIN: &str = "عيدها";
    pub const BACK: &str = "رجوع للقائمة";
    pub const UNVERIFIED: &str = "لسه ما اتراجعش";
    pub const UPDATE: &str = "تحديث عن الكتاب";
}

pub mod calc {
    pub const PICK: &str = "اختار حاسبة";
    pub const FILL: &str = "املا البيانات وهتطلع النتيجة هنا.";
    pub const MISSING: &str = "ناقص";
    pub const INVALID: &str = "راجع القيمة دي";
    pub const OPTIONAL: &str = "اختياري";
    pub const YES: &str = "أيوه";
    pub const NO: &str = "لأ";
    pub const RESULT: &str = "النتيجة";
    pub const BREAKDOWN: &str = "التفاصيل";
    pub const SOURCE: &str = "المصدر";
    pub const BOOK_PAGE: &str = "الكتاب صفحة";
    pub const RESET: &str = "امسح";
    pub const BACK: &str = "كل الحاسبات";
    pub const AREA_LIVER: &str = "الكبد";
    pub const AREA_ASCITES: &str = "الاستسقاء";
    pub const AREA_TUMOUR: &str = "الأورام";
    pub const AREA_ACUTE: &str = "الفشل الكبدي الحاد";
    pub const AREA_PANCREAS: &str = "البنكرياس";
    // The `calc` item: compute it, then interpret it.
    pub const YOUR_RESULT: &str = "حسبتها كام؟";
    pub const YOUR_RESULT_HINT: &str = "اكتب الرقم أو المرحلة، أو سيبها فاضية واستخدم الحاسبة.";
    pub const CHECK: &str = "اتأكد";
    pub const USE_CALC: &str = "احسبها بالحاسبة";
    pub const MATCH: &str = "مظبوط.";
    pub const NO_MATCH: &str = "مش مظبوط. شوف الحساب تحت.";
    pub const MEANING: &str = "يعني إيه النتيجة دي؟";
    pub const MEANING_HINT: &str = "قولها لنفسك الأول، وبعدين افتح التفسير.";
    pub const SHOW_MEANING: &str = "ورّيني التفسير";
}

pub mod settings {
    pub const TITLE: &str = "الإعدادات";
    pub const CLOSE: &str = "اقفل";
    pub const BACK: &str = "رجوع";
    pub const SECTIONS: &str = "أقسام الإعدادات";
    pub const THIS_DEVICE: &str = "للجهاز ده بس";

    pub const LOOK: &str = "الشكل";
    pub const LOOK_SUB: &str = "الألوان وحجم الخط";
    pub const DATA: &str = "بياناتك";
    pub const DATA_SUB: &str = "فين بتتحفظ، والتصدير";
    pub const ABOUT: &str = "عن باطنة";
    pub const ABOUT_SUB: &str = "النسخة والمحتوى";

    pub const THEME: &str = "الألوان";
    pub const THEME_SYSTEM: &str = "زي الجهاز";
    pub const THEME_SYSTEM_SUB: &str = "فاتح بالنهار، وغامق لما الجهاز يبقى غامق";
    pub const THEME_PAPER: &str = "ورق";
    pub const THEME_PAPER_SUB: &str = "فاتح وهادي";
    pub const THEME_NIGHT: &str = "ليل";
    pub const THEME_NIGHT_SUB: &str = "غامق";
    pub const THEME_SHIFT: &str = "نبطشية";
    pub const THEME_SHIFT_SUB: &str = "غامق ودافي من غير ضوء أزرق، للمذاكرة متأخر قبل النوم";

    pub const TEXT_SIZE: &str = "حجم الخط";
    pub const TEXT_SMALL: &str = "صغير";
    pub const TEXT_NORMAL: &str = "عادي";
    pub const TEXT_LARGE: &str = "كبير";
    pub const TEXT_LARGER: &str = "أكبر";
    pub const TEXT_SAMPLE_AR: &str = "إيه أول علاج لـ";
    pub const TEXT_SAMPLE_TERM: &str = "variceal bleeding";

    pub const DATA_FOLDER: &str = "فولدر البيانات";
    pub const DATA_NOWHERE: &str = "مش معروف على الجهاز ده";
    pub const DATA_LOCAL: &str = "كل اللي بتذاكره متسجل على الجهاز ده. النسخة الاحتياطية والمزامنة جايين.";

    pub const VERSION: &str = "النسخة";
    pub const CONTENT: &str = "المحتوى";
    pub const LECTURES_WRITTEN: &str = "محاضرة مكتوبة من";
    pub const ITEMS: &str = "سؤال وكارت";
    pub const SOURCE: &str = "المصدر";
    pub const BOOK: &str =
        "Internal Medicine Postgraduate Lectures, Alexandria Faculty of Medicine (2025), part 1";
    pub const PERSONAL: &str = "نسخة شخصية، مش منشورة.";

    pub const DIAG: &str = "اللي حصل من مشاكل";
    pub const DIAG_BODY: &str = "لو حاجة اشتغلت غلط، انسخ السطور دي وابعتها. مفيهاش أي حاجة من مذاكرتك.";
    pub const DIAG_EMPTY: &str = "مفيش مشاكل لحد دلوقتي.";
    pub const DIAG_LINES: &str = "سطر";
    pub const DIAG_REFRESH: &str = "حدّث";
    pub const DIAG_COPY: &str = "انسخ";
    pub const DIAG_COPIED: &str = "اتنسخ";
}
