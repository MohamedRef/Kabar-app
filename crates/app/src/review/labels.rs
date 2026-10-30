//! Localized scheduling and review labels.

use super::*;

pub(super) fn mm_ss(ui: &Ui, ms: u64) -> String {
    let secs = ms / 1000;
    ui.digits(&format!("{}:{:02}", secs / 60, secs % 60))
}
/// A span in words. Arabic counts: 1 and 2 have their own forms, 3–10 take
/// the plural, 11+ the singular. `after_prep`: MSA duals after «بعد».
pub(super) fn span_text(ui: &Ui, span: Span, after_prep: bool) -> String {
    let lang = ui.lang();
    let (n, unit) = match span {
        Span::Minutes(n) => (n, 0),
        Span::Hours(n) => (n, 1),
        Span::Days(n) => (n, 2),
        Span::Months(n) => (n, 3),
        Span::Years(n) => (n, 4),
    };
    let d = ui.num(n);
    let arabic = |forms: [(&str, &str, &str, &str); 5]| {
        let (one, two, few, many) = forms[unit];
        match n {
            1 => one.to_string(),
            2 => two.to_string(),
            3..=10 => format!("{d} {few}"),
            _ => format!("{d} {many}"),
        }
    };
    match lang {
        UiLanguage::En => {
            let (one, many) = [
                ("minute", "minutes"),
                ("hour", "hours"),
                ("day", "days"),
                ("month", "months"),
                ("year", "years"),
            ][unit];
            format!("{d} {}", if n == 1 { one } else { many })
        }
        UiLanguage::ArEg => arabic([
            ("دقيقة", "دقيقتين", "دقايق", "دقيقة"),
            ("ساعة", "ساعتين", "ساعات", "ساعة"),
            ("يوم", "يومين", "أيام", "يوم"),
            ("شهر", "شهرين", "شهور", "شهر"),
            ("سنة", "سنتين", "سنين", "سنة"),
        ]),
        UiLanguage::Ar if after_prep => arabic([
            ("دقيقة", "دقيقتين", "دقائق", "دقيقة"),
            ("ساعة", "ساعتين", "ساعات", "ساعة"),
            ("يوم", "يومين", "أيام", "يومًا"),
            ("شهر", "شهرين", "أشهر", "شهرًا"),
            ("سنة", "سنتين", "سنوات", "سنة"),
        ]),
        UiLanguage::Ar => arabic([
            ("دقيقة", "دقيقتان", "دقائق", "دقيقة"),
            ("ساعة", "ساعتان", "ساعات", "ساعة"),
            ("يوم", "يومان", "أيام", "يومًا"),
            ("شهر", "شهران", "أشهر", "شهرًا"),
            ("سنة", "سنتان", "سنوات", "سنة"),
        ]),
    }
}
/// Compact interval for an answer button: "10m · 1d · 3d" in English.
pub(super) fn span_short(ui: &Ui, span: Span) -> String {
    if ui.settings.read().ui_language != UiLanguage::En {
        return span_text(ui, span, false);
    }
    match span {
        Span::Minutes(n) => format!("{n}m"),
        Span::Hours(n) => format!("{n}h"),
        Span::Days(n) => format!("{n}d"),
        Span::Months(n) => format!("{n}mo"),
        Span::Years(n) => format!("{n}y"),
    }
}
/// "in 10 minutes", "tomorrow", "in 3 days".
pub(super) fn when_text(ui: &Ui, sched: &Scheduler, now: u64, due: u64) -> String {
    match Span::until(sched, now, due) {
        Span::Days(1) => ui.t(Msg::RevTomorrow).to_string(),
        span => ui.t(Msg::RevIn).replace("{span}", &span_text(ui, span, true)),
    }
}
pub(super) fn weekday(ui: &Ui, day: i64) -> &'static str {
    // Day 0 of the learner's calendar (1 Jan 1970) was a Thursday.
    let i = (day + 4).rem_euclid(7) as usize;
    ui.t([
        Msg::SunShort,
        Msg::MonShort,
        Msg::TueShort,
        Msg::WedShort,
        Msg::ThuShort,
        Msg::FriShort,
        Msg::SatShort,
    ][i])
}
pub(super) fn rating_msg(r: Rating) -> Msg {
    match r {
        Rating::Again => Msg::RateAgain,
        Rating::Hard => Msg::RateHard,
        Rating::Good => Msg::RateGood,
        Rating::Easy => Msg::RateEasy,
    }
}
