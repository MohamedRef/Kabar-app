use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Message {
    Title,
    Previous,
    Next,
    Today,
    Planned,
    Completed,
    Deadline,
    TargetDay,
    Practice,
    BeyondPlan,
}
}

impl crate::i18n::Text for Message {
    fn text(self, lang: UiLanguage) -> &'static str {
        use Message::*;
        use UiLanguage::*;
        match (self, lang) {
            (Title, ArEg) => "تقويم المذاكرة",
            (Title, Ar) => "تقويم الدراسة",
            (Title, En) => "Study calendar",
            (Previous, ArEg) => "الشهر اللي فات",
            (Previous, Ar) => "الشهر السابق",
            (Previous, En) => "Previous month",
            (Next, ArEg) => "الشهر الجاي",
            (Next, Ar) => "الشهر التالي",
            (Next, En) => "Next month",
            (Today, ArEg) => "النهارده",
            (Today, Ar) => "اليوم",
            (Today, En) => "Today",
            (Planned, ArEg | Ar) => "مخطط",
            (Planned, En) => "Planned",
            (Completed, ArEg) => "اتعمل",
            (Completed, Ar) => "مكتمل",
            (Completed, En) => "Completed",
            (Deadline, ArEg | Ar) => "موعد الهدف",
            (Deadline, En) => "Target date",
            (TargetDay, ArEg | Ar) => "يوم الهدف",
            (TargetDay, En) => "Target day",
            (Practice, ArEg) => "مراجعة وتمارين",
            (Practice, Ar) => "مراجعة وتدريب",
            (Practice, En) => "Review and practice",
            (BeyondPlan, ArEg | Ar) => "وقت الدروس خارج الخطة",
            (BeyondPlan, En) => "Lesson time beyond the plan",
        }
    }
}

pub(super) fn progress(lang: UiLanguage, assigned: usize, done: usize) -> String {
    match lang {
        UiLanguage::ArEg => format!("{assigned} دروس مقررة · {done} دروس خلصت"),
        UiLanguage::Ar => format!("{assigned} دروس مقررة · {done} دروس مكتملة"),
        UiLanguage::En => format!("{assigned} planned lessons · {done} finished lessons"),
    }
}

pub(super) fn summary(lang: UiLanguage, done: usize, duration: &str, reviews: u32) -> String {
    match lang {
        UiLanguage::ArEg => format!("{done} دروس خلصت · {duration} مذاكرة · {} مراجعات", reviews),
        UiLanguage::Ar => format!("{done} دروس مكتملة · {duration} دراسة · {} مراجعات", reviews),
        UiLanguage::En => format!("{done} lessons finished · {duration} studied · {} reviews", reviews),
    }
}

#[cfg(test)]
mod tests;
