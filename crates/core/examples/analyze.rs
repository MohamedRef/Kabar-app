//! Prints the deterministic analysis of a text: `cargo run -p kabar-core --example analyze -- "arp dicul piro"`.

use kabar_core::lang::analyze::{Engine, Morph};

fn main() {
    let text = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    for t in Engine::default().analyze(&text) {
        let morph = match &t.morph {
            Some(Morph::Indonesian(a)) => a.display(),
            Some(Morph::Javanese(a)) => {
                format!("{}{}", a.display(), if a.verified { "" } else { " (unverified)" })
            }
            None => String::new(),
        };
        let norm = t.normalized.as_ref().map(|n| format!("→ {}", n.full)).unwrap_or_default();
        println!(
            "{:<14} {:<12} {:<10?} {:.2} {:<10?} {}",
            t.token.text, norm, t.lang.lang, t.lang.confidence, t.lang.source, morph
        );
    }
}
