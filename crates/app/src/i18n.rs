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
    pub const RATE_YOURSELF: &str = "قيّم نفسك";
    pub const SUGGESTED: &str = "اقتراحنا";
    pub const AGAIN: &str = "تاني";
    pub const HARD: &str = "صعبة";
    pub const GOOD: &str = "تمام";
    pub const EASY: &str = "سهلة";

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
