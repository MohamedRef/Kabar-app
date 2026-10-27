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
