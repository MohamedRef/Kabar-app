# Settings browser regression

The dedicated `Settings browser regression` workflow exports the actual Dioxus
`SettingsSheet`, section index and Look controls through the test-only SSR
renderer. It uses the app's production stylesheets, bundled fonts, theme
implementation and preference types. There is no copied Settings HTML or CSS.

The browser uses Chromium and the production `assets/settings-dialog.js`.
Playwright is pinned to 1.62.1 in this test job; it is not an app dependency.
No real user data, live APIs, app launch, installation or release is involved.

| Dimension | Cases |
|---|---|
| Width | 320, 384, 600, 840, 1280 CSS px |
| Height | 900 CSS px |
| Theme | Every supported theme; System variants in light and dark OS schemes |
| Appearance | Calm and Glossy |
| Text size | Normal and the largest supported size |
| Direction | Egyptian Arabic RTL and English LTR |

The matrix runs 400 browser cases. The text-scale assertion respects the\nproduction 15px compact and 16px wider normal bases, and checks the 125%\nlarger-text scaling rather than enforcing one size at every width.\nAssertions inspect computed dialog bounds,
document and pane overflow, the responsive section index and Back target, actual
focus after Tab/Shift+Tab, the Escape target, inert background and focus restored
after removal. Browser errors fail the job. Largest-text Calm screenshots and a
JSON report are attached as `settings-browser-results`; failures attach a screenshot.

## Run

```sh
APP_BROWSER_FIXTURES="$PWD/target/browser-fixtures" cargo test -p kabar-app --no-default-features --locked export_browser_fixtures -- --ignored
# Install the pinned Playwright package and Chromium in a separate tool folder.
NODE_PATH=/path/to/browser-tools/node_modules node crates/app/tests/settings-browser.cjs target/browser-fixtures target/browser-results
```

## Scope and remaining checks

The fixture is server rendered. Rust event callbacks are not hydrated, so the
suite does not assert application click/navigation/persistence behavior. It
observes the Escape click target and simulates DOM removal only to verify the
controller's cleanup. Existing Rust and production-controller Node tests retain
their separate responsibilities.

This adds real computed-layout and keyboard coverage for Settings > Look.
It is not an end-to-end test of every screen or a physical Mac, Android WebView,
screen-reader, S Pen, TalkBack or VoiceOver result. Those checks remain in the
device matrix. A passing Chromium check must not be reported as device QA.
