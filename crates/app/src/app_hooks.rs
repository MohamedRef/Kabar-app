//! The app-wide hooks `Loaded` starts: cloud sync, learner profile, window width, dark mode and the share inbox.

use super::*;

/// Starts Kabar Cloud sync (when this build knows a server) and mirrors its
/// events into signals.
pub(super) fn use_cloud(db: &Db, path: std::path::PathBuf, mut ui: Ui, link: cloud::link::CloudLink) {
    let device_id = db.0.borrow().device_id().clone();
    let cloud = use_context_provider(|| Cloud {
        status: Signal::new(CloudStatus::not_configured(device_id.clone())),
        devices: Signal::new(None),
        usage: Signal::new(None),
        signing_in: Signal::new(false),
        login: Signal::new(Default::default()),
        revision: Signal::new(0),
        commands: Signal::new(None),
        push: Signal::new(platform::push::pushed()),
        push_sent: Signal::new(None),
    });
    let db = db.clone();
    let coach = try_use_context::<Coach>();
    let words = use_context::<annotated::Words>();
    use_hook(move || {
        let Some(config) = cloud::CloudConfig::from_env() else { return };
        let device = kabar_protocol::cloud::DeviceInfo {
            device_id,
            name: platform::device_name().unwrap_or_else(|| "Kabar".into()),
            model: platform::device_model(),
            platform: if cfg!(target_os = "android") {
                kabar_protocol::cloud::Platform::Android
            } else if cfg!(target_arch = "wasm32") {
                kabar_protocol::cloud::Platform::Web
            } else if cfg!(target_os = "macos") {
                kabar_protocol::cloud::Platform::Macos
            } else {
                kabar_protocol::cloud::Platform::Other
            },
        };
        #[cfg(target_os = "macos")]
        let vault: Box<dyn cloud::session::SessionVault> = match path.parent() {
            Some(dir) => Box::new(cloud::session::FileVault::mac(dir)),
            None => Box::new(cloud::session::KeyringVault::new()),
        };
        #[cfg(all(not(target_os = "macos"), not(target_arch = "wasm32")))]
        let vault: Box<dyn cloud::session::SessionVault> = Box::new(cloud::session::KeyringVault::new());
        #[cfg(not(target_arch = "wasm32"))]
        let started = cloud::start(config, path, device, vault);
        #[cfg(target_arch = "wasm32")]
        let started = {
            let _ = path;
            cloud::start(config, db.clone(), device, Box::new(cloud::session::WebVault::kept()))
        };
        let mut handle = match started {
            Ok(h) => h,
            Err(e) => {
                crate::diag::report!("sync didn't start: {e}");
                return;
            }
        };
        let mut commands = cloud.commands;
        commands.set(Some(handle.commands.clone()));
        link.connect(handle.commands.clone());
        spawn(async move {
            let (mut status, mut devices, mut signing_in, mut revision, mut login) =
                (cloud.status, cloud.devices, cloud.signing_in, cloud.revision, cloud.login);
            let (mut usage, mut push, mut push_sent) = (cloud.usage, cloud.push, cloud.push_sent);
            while let Some(event) = handle.events.recv().await {
                match event {
                    Event::Status(s) => {
                        if *status.peek() != s {
                            status.set(s);
                        }
                    }
                    Event::Devices(d) => devices.set(Some(d)),
                    Event::Usage(u) => usage.set(Some(u)),
                    Event::SigningIn(on) => signing_in.set(on),
                    Event::Login(l) => login.set(l),
                    Event::Push { token, pushed: Some(pushed) } => {
                        platform::push::set_pushed(&token, pushed);
                        if *push.peek() != pushed {
                            push.set(pushed);
                        }
                    }
                    // Not answered: try again on the next reconnect.
                    Event::Push { pushed: None, .. } => push_sent.set(None),
                    Event::Changed(tables) => {
                        let store = db.0.borrow();
                        if tables.contains("settings")
                            && let Ok(s) = store.settings()
                        {
                            ui.settings.set(s);
                        }
                        // Kak Nur's rows: goal met on any device counts here too.
                        if let Some(mut coach) = coach
                            && ["goals", "daily_plans", "xp_events", "reminders", "reports"]
                                .iter()
                                .any(|t| tables.contains(*t))
                        {
                            coach.profile.set(store.coach_profile().ok().flatten());
                            coach.events.set(store.activity(None).unwrap_or_default());
                            coach.reminders.set(store.reminder_state().unwrap_or_default());
                            coach.plan_rev += 1;
                        }
                        if tables.contains("taught_entries") {
                            drop(store);
                            words.refresh(&db);
                        }
                        revision += 1;
                    }
                }
            }
        });
    });
    // Local settings writes are in the outbox: push them.
    use_effect(move || {
        ui.settings.read();
        cloud.send(Command::Kick);
    });
}

/// Mirrors the window width into `width`, so layout follows the window,
/// not the device (split screen, DeX and Mac windows all just work).
/// Keeps the AI prompts' learner profile current: it follows the activity
/// log (lessons, reviews, practice) and the onboarding answers. The minute
/// clock is only peeked, so the deck isn't re-read every minute.
pub(super) fn use_learner_profile(db: &Db, coach: Coach) {
    let ai = use_context::<teach::Ai>();
    let db = db.clone();
    use_effect(move || {
        let events = coach.events.read();
        let profile = coach.profile.read();
        let offset = profile
            .as_ref()
            .map(|p| p.utc_offset_min)
            .or(*coach.device_offset.peek())
            .unwrap_or(kabar_protocol::coach::CoachProfile::default().utc_offset_min);
        let today = coach::clock::today(*coach.now_ms.peek(), offset);
        ai.0.set_profile(ai::learner_profile(&db, profile.as_ref(), &events, today));
    });
}

pub(super) fn use_width_tracking(mut width: Signal<WidthClass>, mut tablet: Signal<bool>) {
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            const send = () => dioxus.send([window.innerWidth, Math.min(screen.width, screen.height)]);
            window.addEventListener("resize", send);
            send();
            await new Promise(() => {});
            "#,
        );
        while let Ok((w, smallest)) = eval.recv::<(f64, f64)>().await {
            let class = WidthClass::from_width(w);
            if *width.peek() != class {
                width.set(class);
            }
            let is_tablet = cfg!(target_os = "android") && smallest >= 600.0;
            if *tablet.peek() != is_tablet {
                tablet.set(is_tablet);
            }
        }
    });
}

/// System themes follow the device's dark mode. Android's WebView can't be
/// trusted for it (it reports light under a light activity theme), so there
/// the platform is asked directly and `data-theme` names the concrete side.
/// It's asked again only while a System theme is on and the app is on
/// screen: when it comes back, gains focus, the WebView sees a scheme
/// change, or every few seconds (a quick-settings toggle fires none of
/// those). Elsewhere it stays None and `prefers-color-scheme` does the job.
pub(super) fn use_device_dark(ui: Ui) -> Signal<Option<bool>> {
    let device_dark = use_signal(platform::device_dark);
    use_future(move || async move {
        if !cfg!(target_os = "android") {
            return;
        }
        let mut eval = document::eval(
            r#"
            const ask = () => { if (!document.hidden) dioxus.send(true); };
            document.addEventListener("visibilitychange", ask);
            window.addEventListener("focus", ask);
            matchMedia("(prefers-color-scheme: dark)").addEventListener("change", ask);
            setInterval(ask, 5000);
            await new Promise(() => {});
            "#,
        );
        while eval.recv::<bool>().await.is_ok() {
            refresh(ui, device_dark);
        }
    });
    // Picking a System theme asks at once instead of on the next tick.
    use_effect(move || {
        ui.settings.read();
        refresh(ui, device_dark);
    });
    device_dark
}

pub(super) fn refresh(ui: Ui, mut device_dark: Signal<Option<bool>>) {
    if !cfg!(target_os = "android") || ui.settings.peek().theme.follows_os().is_none() {
        return;
    }
    let now = platform::device_dark();
    if now.is_some() && now != *device_dark.peek() {
        device_dark.set(now);
    }
}

/// Follows the window to the back and front: sync sleeps in the background,
/// and text or images shared to Kabar (Android share sheet) open the Reader
/// at start and whenever the window comes back.
pub(super) fn use_share_inbox(ui: Ui, inbox: reader::Inbox) {
    let cloud = use_context::<Cloud>();
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            const send = () => dioxus.send(!document.hidden);
            document.addEventListener("visibilitychange", send);
            window.addEventListener("focus", send);
            send();
            await new Promise(() => {});
            "#,
        );
        let mut shown = true;
        while let Ok(visible) = eval.recv::<bool>().await {
            if visible != shown {
                shown = visible;
                cloud.send(Command::Background(!visible));
            }
            if !visible {
                continue;
            }
            if let Some(input) = platform::take_shared_input() {
                let (mut slot, mut screen) = (inbox.0, ui.screen);
                slot.set(Some(input));
                screen.set(Screen::Reader);
            }
        }
    });
}
