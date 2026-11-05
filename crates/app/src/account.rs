//! Account & devices: Google sign-in, sync status, the Devices list with
//! remote sign-out, and a small "save a word" box to watch sync happen.

use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, format_number};
use kabar_protocol::cloud::{DeviceSummary, Platform, UsageReport};
use kabar_protocol::sync::OpKind;
use tokio::sync::mpsc::UnboundedSender;

use crate::account::msg::AccountMsg;
use crate::cloud::{CloudStatus, Command, LoginStatus, Problem, Secret, SyncState};
use crate::i18n::Msg;
use crate::{Db, Ui};

pub mod msg;

/// Kabar Cloud state for the UI, shared through context.
#[derive(Clone, Copy)]
pub struct Cloud {
    pub status: Signal<CloudStatus>,
    pub devices: Signal<Option<Result<Vec<DeviceSummary>, Problem>>>,
    /// The cost dashboard's counters, once asked for.
    pub usage: Signal<Option<Result<UsageReport, Problem>>>,
    pub signing_in: Signal<bool>,
    pub login: Signal<LoginStatus>,
    /// Bumped when remote ops change rows the UI shows.
    pub revision: Signal<u64>,
    pub commands: Signal<Option<UnboundedSender<Command>>>,
    /// Kabar Cloud pushes Kak Nur's reminders to this device (FCM, #187), so
    /// it schedules none of its own. Kept across launches by `PushBridge`.
    pub push: Signal<bool>,
    /// The token last sent this run (`""`: "not here"), so it goes once.
    pub push_sent: Signal<Option<String>>,
}

impl Cloud {
    pub fn send(&self, command: Command) {
        if let Some(tx) = &*self.commands.peek() {
            tx.send(command).ok();
        }
    }
}

pub fn problem_msg(p: Problem) -> Msg {
    match p {
        Problem::NotConfigured => Msg::CloudNotConfigured,
        Problem::NotAllowed => Msg::ProblemNotAllowed,
        Problem::SignInCancelled => Msg::ProblemSignInCancelled,
        Problem::SignInFailed => Msg::ProblemSignInFailed,
        Problem::SignedOutRemotely => Msg::ProblemSignedOutRemotely,
        Problem::Network => Msg::ProblemNetwork,
        Problem::Server => Msg::ProblemServer,
        Problem::Outdated => Msg::ProblemOutdated,
        Problem::WrongPassword => Msg::ProblemWrongPassword,
        Problem::Locked(_) => Msg::ProblemLocked,
        Problem::UsernameTaken => Msg::ProblemTaken,
        Problem::BadUsername => Msg::ProblemBadUsername,
        Problem::ShortPassword => Msg::ProblemShortPassword,
        Problem::Mismatch => Msg::ProblemMismatch,
    }
}

/// A failed password sign-in, said in the card itself: it isn't retried in
/// the background, so "I'll keep trying" would promise a retry that never
/// comes (#579).
pub fn sign_in_problem_line(ui: &Ui, p: Problem) -> String {
    match p {
        Problem::Network => ui.t(Msg::ProblemSignInNetwork).to_string(),
        Problem::Server => ui.t(AccountMsg::ProblemSignInServer).to_string(),
        p => problem_line(ui, p),
    }
}

/// Set once a password sign-in was sent from this screen, so its outcome
/// shows inside the sign-in card instead of under the card above it.
#[derive(Clone, Copy)]
struct PasswordTried(Signal<bool>);

/// The sentence for a problem, with the wait when the username is locked.
pub fn problem_line(ui: &Ui, p: Problem) -> String {
    match p {
        Problem::Locked(minutes) => ui.fill(Msg::ProblemLocked, &[("n", &ui.num(minutes))]),
        p => ui.t(problem_msg(p)).to_string(),
    }
}

pub(crate) fn sync_msg(s: SyncState) -> Option<Msg> {
    match s {
        SyncState::SignedOut => None,
        SyncState::Connecting => Some(Msg::SyncConnecting),
        SyncState::CatchingUp => Some(Msg::SyncCatchingUp),
        SyncState::Live => Some(Msg::SyncLive),
        SyncState::Offline => Some(Msg::SyncOffline),
        SyncState::Stopped => Some(Msg::SyncStopped),
    }
}

/// "3 min ago" with Arabic dual and plural forms.
pub fn ago(lang: UiLanguage, eastern: bool, then_ms: u64, now_ms: u64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        return crate::i18n::t(lang, AccountMsg::JustNow).to_string();
    }
    let (n, unit) = match secs {
        s if s < 3600 => (s / 60, 0),
        s if s < 86_400 => (s / 3600, 1),
        s => (s / 86_400, 2),
    };
    let num = format_number(n, lang, eastern);
    match lang {
        UiLanguage::En => {
            let unit = ["min", "h", "d"][unit];
            format!("{num} {unit} ago")
        }
        UiLanguage::ArEg | UiLanguage::Ar => {
            // [one, two, 3–10, 11+]
            let forms = [
                ["دقيقة", "دقيقتين", "دقايق", "دقيقة"],
                ["ساعة", "ساعتين", "ساعات", "ساعة"],
                ["يوم", "يومين", "أيام", "يوم"],
            ][unit];
            let forms = if lang == UiLanguage::Ar && unit == 0 {
                ["دقيقة", "دقيقتين", "دقائق", "دقيقة"]
            } else {
                forms
            };
            let since = if lang == UiLanguage::ArEg { "من" } else { "منذ" };
            match n {
                1 => format!("{since} {}", forms[0]),
                2 => format!("{since} {}", forms[1]),
                3..=10 => format!("{since} {num} {}", forms[2]),
                _ => format!("{since} {num} {}", forms[3]),
            }
        }
    }
}

fn now_ms() -> u64 {
    crate::store::now_ms()
}

/// The Devices screen (from Teach Kabar and Cost & limits): the page
/// heading, then the same account and devices as Settings › Account.
#[component]
pub fn DevicesScreen() -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "page-head",
            div { h1 { "{ui.t(Msg::NavDevices)}" } }
        }
        AccountScreen {}
    }
}

#[component]
pub fn AccountScreen() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let status = cloud.status.read().clone();
    let signed_in = status.email.is_some() || status.sync != SyncState::SignedOut;
    let PasswordTried(tried) = use_context_provider(|| PasswordTried(Signal::new(false)));
    // A password attempt's problem shows in its own card.
    let card_problem = status.problem.filter(|_| signed_in || !tried());
    let intro = if cfg!(target_arch = "wasm32") { AccountMsg::CloudIntroWeb } else { AccountMsg::CloudIntro };

    // Load the device list when this screen opens, and again after sign-in.
    let state = use_memo(move || cloud.status.read().sync);
    use_effect(move || {
        if matches!(state(), SyncState::Live | SyncState::CatchingUp) {
            cloud.send(Command::LoadDevices);
        }
    });

    rsx! {
        crate::ui::DivCard {
            h2 { bdi { lang: "en", class: "latin", "Kabar Cloud" } }
            if !status.configured {
                p { class: "muted", "{ui.t(Msg::CloudNotConfigured)}" }
            } else if signed_in {
                SignedIn { status: status.clone() }
            } else {
                // The web app signs in with a username and password only
                // (below), so it doesn't point to a Google button (#578).
                p { "{ui.t(intro)}" }
                if cfg!(target_arch = "wasm32") {
                } else if *cloud.signing_in.read() {
                    p { class: "muted", aria_live: "polite", "{ui.t(AccountMsg::CloudSigningIn)}" }
                } else {
                    button { class: "primary", onclick: move |_| cloud.send(Command::SignInGoogle), "{ui.t(Msg::CloudSignIn)}" }
                }
            }
            if let Some(p) = card_problem {
                p { class: "notice", role: "status", "{problem_line(&ui, p)}" }
            }
        }
        if status.configured && !signed_in {
            PasswordSignIn {}
        }
        if signed_in {
            LoginCard {}
            DevicesCard {}
            TrySyncCard {}
        }
    }
}

#[component]
fn SignedIn(status: CloudStatus) -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let settings = ui.settings.read().clone();
    let email = status.email.clone().unwrap_or_default();
    let last = status.last_sync_ms.map(|ms| ago(settings.ui_language, settings.eastern_digits, ms, now_ms()));
    let pending = format_number(status.pending, settings.ui_language, settings.eastern_digits);

    rsx! {
        div { class: "field",
            span { class: "label", "{ui.t(AccountMsg::CloudSignedInAs)}" }
            bdi { class: "latin", "{email}" }
        }
        if status.sync == SyncState::Offline {
            crate::misc_art::Picture { art: crate::misc_art::Art::Offline, class: "empty-art small" }
        }
        if status.temporary {
            p { class: "notice", role: "note", "{ui.t(AccountMsg::PwTemporary)}" }
        }
        div { class: "field row",
            if let Some(msg) = sync_msg(status.sync) {
                span {
                    class: if status.sync == SyncState::Live { "pill ok" } else { "pill" },
                    aria_live: "polite",
                    "{ui.t(msg)}"
                }
            }
            if status.pending > 0 {
                span { class: "muted", "{pending} {ui.t(AccountMsg::SyncPending)}" }
            }
            if let Some(last) = last {
                span { class: "muted", "{ui.t(AccountMsg::SyncLast)}: {last}" }
            }
        }
        div { class: "sync-actions",
            button { onclick: move |_| cloud.send(Command::SignOut), "{ui.t(AccountMsg::CloudSignOut)}" }
        }
    }
}

/// First launch on a new device (the Welcome screen): someone who already
/// learns with Kabar signs in instead of starting over. The account's
/// profile and lessons sync down before the hub says Live, so Today opens
/// where they left off, and this device's onboarding never overwrites them.
#[component]
pub fn ReturningSignIn() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let cloud = use_context::<Cloud>();
    let mut open = use_signal(|| false);
    let status = cloud.status.read().clone();
    let PasswordTried(tried) = use_context_provider(|| PasswordTried(Signal::new(false)));
    let state = use_memo(move || cloud.status.read().sync);
    use_effect(move || {
        if state() == SyncState::Live && !ui.settings.peek().welcomed {
            ui.update_settings(&db, |s| s.welcomed = true);
        }
    });
    if !status.configured {
        return rsx! {};
    }
    rsx! {
        if !open() {
            p {
                button { class: "outline", onclick: move |_| open.set(true), "{ui.t(AccountMsg::WelcomeHaveAccount)}" }
            }
        } else {
            div { class: "welcome-sign-in",
                if !cfg!(target_arch = "wasm32") {
                    p {
                        button {
                            disabled: *cloud.signing_in.read(),
                            onclick: move |_| cloud.send(Command::SignInGoogle),
                            "{ui.t(Msg::CloudSignIn)}"
                        }
                    }
                }
                // Signed in already (the account is syncing down): a second
                // sign-in would only replace the session it just got.
                if status.sync == SyncState::SignedOut {
                    PasswordSignIn {}
                }
                // A password attempt's problem shows inside its card.
                if let Some(p) = status.problem.filter(|_| !tried() || status.sync != SyncState::SignedOut) {
                    p { class: "notice", role: "status", "{problem_line(&ui, p)}" }
                }
            }
        }
    }
}

/// Signed out: username + password, for a device without Google and the web.
#[component]
fn PasswordSignIn() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut theirs = use_signal(|| false);
    let local = use_signal(|| false);
    let mut tried = try_use_context::<PasswordTried>().map_or(local, |t| t.0);
    let busy = *cloud.signing_in.read();
    let problem = cloud.status.read().problem.filter(|_| tried() && !busy);
    let ready = !username.read().trim().is_empty() && !password.read().is_empty();
    rsx! {
        crate::ui::DivCard { class: "pw-card",
            h2 { "{ui.t(AccountMsg::PwTitle)}" }
            p { class: "muted", "{ui.t(AccountMsg::PwIntro)}" }
            form { class: "pw-form",
                onsubmit: move |e| {
                    e.prevent_default();
                    if !ready || busy {
                        return;
                    }
                    cloud.send(Command::SignInPassword {
                        username: username.peek().clone(),
                        password: Secret(password.peek().clone()),
                        remember: !*theirs.peek(),
                    });
                    // The password stays typed, so a retry is one tap (#579).
                    tried.set(true);
                },
                label { class: "field",
                    span { class: "label", "{ui.t(AccountMsg::PwUsername)}" }
                    input {
                        class: "text-input latin", r#type: "text", name: "username", dir: "ltr", lang: "en",
                        autocomplete: "username", autocapitalize: "none", spellcheck: "false",
                        value: "{username}",
                        oninput: move |e| username.set(e.value()),
                    }
                }
                label { class: "field",
                    span { class: "label", "{ui.t(AccountMsg::PwPassword)}" }
                    input {
                        class: "text-input latin", r#type: "password", name: "password", dir: "ltr",
                        autocomplete: "current-password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }
                }
                label { class: "pw-theirs",
                    input {
                        r#type: "checkbox",
                        checked: theirs(),
                        onchange: move |e: FormEvent| theirs.set(e.checked()),
                    }
                    span {
                        "{ui.t(AccountMsg::PwNotMine)}"
                        small { class: "muted", "{ui.t(AccountMsg::PwNotMineHint)}" }
                    }
                }
                div { class: "sync-actions",
                    button { class: "primary", r#type: "submit", disabled: !ready || busy,
                        if busy { "{ui.t(AccountMsg::PwChecking)}" } else { "{ui.t(AccountMsg::PwSignIn)}" }
                    }
                }
                if let Some(p) = problem {
                    p { class: "field-error", role: "alert", "{sign_in_problem_line(&ui, p)}" }
                }
            }
        }
    }
}

/// Signed in: the username and password this account signs in with
/// elsewhere, set or changed here.
#[component]
fn LoginCard() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let login = cloud.login.read().clone();
    let state = use_memo(move || cloud.status.read().sync);
    use_effect(move || {
        if matches!(state(), SyncState::Live | SyncState::CatchingUp) && cloud.login.peek().username.is_none()
        {
            cloud.send(Command::LoadLogin);
        }
    });
    let current = login.username.clone().flatten();
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut again = use_signal(String::new);
    let mut mismatch = use_signal(|| false);
    let mut editing = use_signal(|| false);
    // The form starts from the username the account has.
    let known = current.clone();
    use_effect(use_reactive!(|known| {
        if let Some(k) = known
            && username.peek().is_empty()
        {
            username.set(k);
        }
    }));
    let saved = login.saved;
    use_effect(use_reactive!(|saved| {
        if saved {
            password.set(String::new());
            again.set(String::new());
            editing.set(false);
        }
    }));
    let open = editing() || login.username == Some(None);
    let ready = !username.read().trim().is_empty() && !password.read().is_empty() && !again.read().is_empty();
    let problem = if mismatch() { Some(Problem::Mismatch) } else { login.problem };
    rsx! {
        crate::ui::DivCard { class: "pw-card",
            h2 { "{ui.t(AccountMsg::PwTitle)}" }
            match &login.username {
                None => rsx! { p { class: "muted", "…" } },
                Some(Some(name)) => rsx! {
                    div { class: "field",
                        span { class: "label", "{ui.t(AccountMsg::PwYours)}" }
                        bdi { class: "latin", "{name}" }
                    }
                    if saved {
                        p { class: "notice", role: "status", "{ui.t(AccountMsg::PwSaved)}" }
                    }
                    p { class: "muted", "{ui.t(AccountMsg::PwHow)}" }
                },
                Some(None) => rsx! { p { class: "muted", "{ui.t(AccountMsg::PwNone)}" } },
            }
            if open {
                form { class: "pw-form",
                    onsubmit: move |e| {
                        e.prevent_default();
                        if *password.peek() != *again.peek() {
                            mismatch.set(true);
                            return;
                        }
                        mismatch.set(false);
                        cloud.send(Command::SetLogin {
                            username: username.peek().clone(),
                            password: Secret(password.peek().clone()),
                        });
                    },
                    label { class: "field",
                        span { class: "label", "{ui.t(AccountMsg::PwUsername)}" }
                        input {
                            class: "text-input latin", r#type: "text", name: "username", dir: "ltr", lang: "en",
                            autocomplete: "username", autocapitalize: "none", spellcheck: "false",
                            value: "{username}",
                            oninput: move |e| username.set(e.value()),
                        }
                        p { class: "field-hint", "{ui.t(AccountMsg::PwUsernameHint)}" }
                    }
                    label { class: "field",
                        span { class: "label", "{ui.t(AccountMsg::PwNew)}" }
                        input {
                            class: "text-input latin", r#type: "password", name: "new-password", dir: "ltr",
                            autocomplete: "new-password",
                            value: "{password}",
                            oninput: move |e| { password.set(e.value()); mismatch.set(false); },
                        }
                        p { class: "field-hint", "{ui.t(AccountMsg::PwPasswordHint)}" }
                    }
                    label { class: "field",
                        span { class: "label", "{ui.t(AccountMsg::PwAgain)}" }
                        input {
                            class: "text-input latin", r#type: "password", name: "new-password-again", dir: "ltr",
                            autocomplete: "new-password",
                            value: "{again}",
                            oninput: move |e| { again.set(e.value()); mismatch.set(false); },
                        }
                    }
                    div { class: "sync-actions",
                        if current.is_some() {
                            button { r#type: "button", onclick: move |_| editing.set(false), "{ui.t(AccountMsg::PwCancel)}" }
                        }
                        button { class: "primary", r#type: "submit", disabled: !ready || login.saving,
                            if login.saving { "{ui.t(AccountMsg::PwSaving)}" } else { "{ui.t(AccountMsg::PwSave)}" }
                        }
                    }
                }
            } else if current.is_some() {
                div { class: "sync-actions",
                    button { class: "danger", disabled: login.saving, onclick: move |_| cloud.send(Command::RemoveLogin), "{ui.t(AccountMsg::PwRemove)}" }
                    button { onclick: move |_| editing.set(true), "{ui.t(AccountMsg::PwChange)}" }
                }
            }
            if let Some(p) = problem {
                p { class: "notice", role: "status", "{problem_line(&ui, p)}" }
            }
        }
    }
}

#[component]
fn DevicesCard() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let settings = ui.settings.read().clone();
    let devices = cloud.devices.read().clone();
    let now = now_ms();

    rsx! {
        crate::ui::DivCard {
            div { class: "row devices-head",
                h2 { "{ui.t(AccountMsg::DevicesTitle)}" }
                button { onclick: move |_| cloud.send(Command::LoadDevices), "{ui.t(Msg::DevicesRefresh)}" }
            }
            match devices {
                None => rsx! { p { class: "muted", "…" } },
                Some(Err(p)) => rsx! { p { class: "notice", "{problem_line(&ui, p)}" } },
                Some(Ok(list)) => rsx! {
                    ul { class: "devices",
                        for d in list {
                            li {
                                key: "{d.device_id}",
                                class: if d.revoked { "device revoked" } else { "device" },
                                div { class: "device-icon", aria_hidden: "true", {platform_icon(d.platform)} }
                                div { class: "device-main",
                                    div { class: "row",
                                        bdi { class: "device-name", "{d.name}" }
                                        if d.current {
                                            span { class: "pill info", "{ui.t(AccountMsg::DeviceThis)}" }
                                        }
                                    }
                                    div { class: "muted",
                                        if let Some(model) = &d.model {
                                            bdi { class: "latin", "{model}" }
                                            " · "
                                        }
                                        if d.revoked {
                                            "{ui.t(AccountMsg::DeviceSignedOut)}"
                                        } else if let Some(ms) = d.last_sync_ms {
                                            "{ui.t(AccountMsg::SyncLast)}: {ago(settings.ui_language, settings.eastern_digits, ms, now)}"
                                        } else {
                                            "{ui.t(AccountMsg::DeviceNeverSynced)}"
                                        }
                                    }
                                }
                                if !d.current && !d.revoked {
                                    button {
                                        class: "device-out",
                                        onclick: {
                                            let id = d.device_id.clone();
                                            move |_| cloud.send(Command::RevokeDevice(id.clone()))
                                        },
                                        "{ui.t(AccountMsg::DeviceSignOut)}"
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

/// Saves a word to `word_status` and lists what this device holds, so a
/// phone → tablet round trip can be watched by eye.
#[component]
fn TrySyncCard() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let cloud = use_context::<Cloud>();
    let mut word = use_signal(String::new);
    // Re-read when remote ops arrive.
    let _ = cloud.revision.read();
    let words: Vec<String> =
        db.0.borrow()
            .synced_rows("word_status")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, p)| p.get("lexeme").and_then(|l| l.as_str()).map(String::from))
            .take(12)
            .collect();

    let mut save = {
        let db = db.clone();
        move || {
            let lexeme = word.peek().trim().to_lowercase();
            if lexeme.is_empty() || lexeme.chars().count() > 64 {
                return;
            }
            let payload = serde_json::json!({ "lexeme": lexeme, "lang": "id", "status": "learning" });
            let result = db.0.borrow_mut().write_synced(
                "word_status",
                &format!("id:{lexeme}"),
                OpKind::Upsert,
                Some(payload),
            );
            match result {
                Ok(_) => {
                    word.set(String::new());
                    let mut rev = cloud.revision;
                    rev += 1;
                    cloud.send(Command::Kick);
                }
                Err(e) => crate::diag::report!("saving the word failed: {e}"),
            }
        }
    };

    rsx! {
        crate::ui::DivCard {
            h2 { "{ui.t(AccountMsg::TrySyncTitle)}" }
            form {
                class: "row",
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    save();
                },
                input {
                    class: "latin",
                    lang: "id",
                    dir: "ltr",
                    placeholder: "rumah",
                    autocapitalize: "off",
                    value: "{word}",
                    oninput: move |e: FormEvent| word.set(e.value()),
                }
                button { class: "primary", r#type: "submit", "{ui.t(AccountMsg::TrySyncSave)}" }
            }
            if !words.is_empty() {
                p { class: "label", "{ui.t(AccountMsg::TrySyncSaved)}" }
                div { class: "row",
                    for w in words {
                        span { key: "{w}", class: "pill latin", lang: "id", bdi { "{w}" } }
                    }
                }
            }
        }
    }
}

fn platform_icon(p: Platform) -> Element {
    let d = match p {
        // phone
        Platform::Android => "M8 3h8v18H8zM11 18h2",
        // laptop
        Platform::Macos => "M5 6h14v9H5zM3 18h18",
        Platform::Web | Platform::Other => "M4 5h16v11H4zM9 20h6M12 16v4",
    };
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    fn signed_out_account() -> crate::testkit::App {
        use crate::testkit::{App, Setup};
        let mut app = App::new(
            || rsx! { AccountScreen {} },
            Setup { sync: Some(SyncState::SignedOut), ..Setup::default() },
        );
        app.with(|| consume_context::<Cloud>().status.write().configured = true);
        app.settle();
        app
    }

    #[test]
    fn a_failed_password_sign_in_says_so_in_its_card_and_keeps_the_password() {
        use crate::testkit::visible;
        let mut app = signed_out_account();
        // A background sync problem (no attempt yet) stays under the cloud card.
        app.with(|| consume_context::<Cloud>().status.write().problem = Some(Problem::Network));
        app.settle();
        let en = |m| crate::i18n::t(UiLanguage::En, m);
        assert!(visible(&app.html()).contains(en(Msg::ProblemNetwork)));
        assert!(!app.html().contains("role=\"alert\""));

        app.with(|| consume_context::<Cloud>().status.write().problem = None);
        let field = |app: &crate::testkit::App, kind: &str| {
            app.targets("input").into_iter().find(|t| t.attr("type") == Some(kind)).expect(kind).el
        };
        let typed = |v: &str| -> Box<dyn std::any::Any> {
            Box::new(dioxus::html::SerializedFormData::new(v.to_string(), Vec::new()))
        };
        let user = field(&app, "text");
        app.dom.dispatch(user, "input", typed("mohamed"), true);
        app.settle();
        let pass = field(&app, "password");
        app.dom.dispatch(pass, "input", typed("secret-pass"), true);
        app.settle();
        app.submit("pw-form");
        // The engine answers: Kabar Cloud is unreachable (#579).
        app.with(|| consume_context::<Cloud>().status.write().problem = Some(Problem::Network));
        app.settle();
        let html = app.html();
        let text = visible(&html);
        assert!(text.contains(en(Msg::ProblemSignInNetwork)), "{text}");
        assert!(!text.contains(en(Msg::ProblemNetwork)), "no 'I'll keep trying' for a password sign-in");
        assert!(html.contains("role=\"alert\""));
        assert!(html.contains("value=\"secret-pass\""), "the typed password stays for a retry");
    }

    #[test]
    fn relative_times_use_arabic_number_forms() {
        let now = 100 * 86_400_000;
        let eg = |ms| ago(UiLanguage::ArEg, false, now - ms, now);
        assert_eq!(eg(10_000), "دلوقتي");
        assert_eq!(eg(MIN), "من دقيقة");
        assert_eq!(eg(2 * MIN), "من دقيقتين");
        assert_eq!(eg(5 * MIN), "من 5 دقايق");
        assert_eq!(eg(25 * MIN), "من 25 دقيقة");
        assert_eq!(eg(3 * 60 * MIN), "من 3 ساعات");
        assert_eq!(ago(UiLanguage::Ar, false, now - 5 * MIN, now), "منذ 5 دقائق");
        assert_eq!(ago(UiLanguage::Ar, true, now - 2 * 86_400_000, now), "منذ يومين");
        assert_eq!(ago(UiLanguage::En, false, now - 5 * MIN, now), "5 min ago");
    }

    #[test]
    fn hours_and_days_count_in_every_language() {
        let now = 400 * 86_400_000;
        const H: u64 = 60 * MIN;
        const D: u64 = 24 * H;
        assert_eq!(ago(UiLanguage::En, false, now - 59_000, now), "just now");
        assert_eq!(ago(UiLanguage::En, false, now - 3 * H, now), "3 h ago");
        assert_eq!(ago(UiLanguage::En, false, now - 40 * D, now), "40 d ago");
        assert_eq!(ago(UiLanguage::ArEg, false, now - H, now), "من ساعة");
        assert_eq!(ago(UiLanguage::ArEg, false, now - 2 * H, now), "من ساعتين");
        assert_eq!(ago(UiLanguage::ArEg, true, now - 12 * H, now), "من ١٢ ساعة");
        assert_eq!(ago(UiLanguage::Ar, false, now - D, now), "منذ يوم");
        assert_eq!(ago(UiLanguage::Ar, false, now - 7 * D, now), "منذ 7 أيام");
        assert_eq!(ago(UiLanguage::Ar, false, now - 30 * D, now), "منذ 30 يوم");
        assert_eq!(ago(UiLanguage::Ar, false, now - 11 * MIN, now), "منذ 11 دقيقة");
        assert_eq!(ago(UiLanguage::ArEg, false, now + MIN, now), "دلوقتي", "a clock running ahead is 'now'");
    }

    #[test]
    fn every_problem_and_sync_state_has_its_own_words() {
        use crate::cloud::{Problem, SyncState};
        let problems = [
            Problem::NotConfigured,
            Problem::NotAllowed,
            Problem::SignInCancelled,
            Problem::SignInFailed,
            Problem::SignedOutRemotely,
            Problem::Network,
            Problem::Server,
            Problem::Outdated,
        ];
        let mut msgs: Vec<Msg> = problems.into_iter().map(problem_msg).collect();
        msgs.dedup();
        assert_eq!(msgs.len(), problems.len());
        assert_eq!(sync_msg(SyncState::SignedOut), None, "signed out shows no sync pill");
        let states = [
            SyncState::Connecting,
            SyncState::CatchingUp,
            SyncState::Live,
            SyncState::Offline,
            SyncState::Stopped,
        ];
        let mut shown: Vec<Msg> = states.into_iter().filter_map(sync_msg).collect();
        shown.dedup();
        assert_eq!(shown.len(), states.len());
    }

    #[test]
    fn each_platform_has_its_own_device_icon() {
        let svg = |p| dioxus::ssr::render_element(platform_icon(p));
        let (phone, mac, web) = (svg(Platform::Android), svg(Platform::Macos), svg(Platform::Web));
        assert!(phone.contains("M8 3h8v18H8z") && mac.contains("M5 6h14v9H5z"), "{phone} {mac}");
        assert_eq!(web, svg(Platform::Other), "the web and other devices share the screen icon");
        assert!(phone != mac && mac != web);
        assert!(web.contains("stroke=\"currentColor\""));
    }
}
