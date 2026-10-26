//! Mind-map generator: "make me a map of *di pasar*". The `mind_map` task
//! writes the words; this validates them and lays them out radially as
//! notebook items the learner then extends with the pen.

use std::f32::consts::TAU;

use kabar_protocol::content::Lang;
use kabar_protocol::ink::{ItemKind, NotebookItem};
use serde_json::{Value, json};

use crate::ai::AiRequest;

/// The model's map (`prompts/mind_map@v1.md`), shaped like the golden set.
pub use crate::ai::tasks::{GlossedWord as MapWord, MapBranch as Branch, MindMap};

/// The `mind_map` request: learning text only (topic, level, known words).
pub fn request(topic: &str, level: &str, known_words: &[String], lang: Lang) -> AiRequest {
    AiRequest::new(json!({
        "topic": topic.trim(),
        "level": level,
        "known_words": known_words.iter().take(40).collect::<Vec<_>>(),
        "lang": lang.code(),
    }))
}

/// Card sizes, page pixels.
const NODE_W: f32 = 150.0;
const NODE_H: f32 = 52.0;
const BRANCH_R: f32 = 260.0;
const WORD_R: f32 = 190.0;

/// Lays the map out around (`cx`, `cy`): the center, branches on a circle,
/// each branch's words fanned outward. Returns nodes and the arrows between
/// them. `new_id` names each item.
pub fn layout(
    map: &MindMap,
    doc: &str,
    cx: f32,
    cy: f32,
    mut new_id: impl FnMut() -> String,
) -> Vec<NotebookItem> {
    let mut items = Vec::new();
    let mut node = |items: &mut Vec<NotebookItem>,
                    label: &str,
                    gloss: Option<String>,
                    parent: Option<&str>,
                    depth: u8,
                    x: f32,
                    y: f32| {
        let id = new_id();
        items.push(NotebookItem {
            id: id.clone(),
            doc: doc.into(),
            kind: ItemKind::MapNode { label: label.into(), gloss, parent: parent.map(String::from), depth },
            x: x - NODE_W / 2.0,
            y: y - NODE_H / 2.0,
            w: NODE_W,
            h: NODE_H,
        });
        if let Some(p) = parent {
            let arrow = new_id();
            items.push(NotebookItem {
                id: arrow,
                doc: doc.into(),
                kind: ItemKind::Arrow { from: Some(p.into()), to: Some(id.clone()), x2: x, y2: y },
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            });
        }
        id
    };
    let center = node(&mut items, &map.center, None, None, 0, cx, cy);
    let n = map.branches.len().max(1) as f32;
    for (i, b) in map.branches.iter().enumerate() {
        let a = -TAU / 4.0 + TAU * i as f32 / n;
        let (bx, by) = (cx + BRANCH_R * a.cos(), cy + BRANCH_R * a.sin());
        let branch = node(&mut items, &b.label, None, Some(&center), 1, bx, by);
        // Fan the words over a slice of the circle facing away from the center.
        let spread = (TAU / n).min(TAU / 3.0) * 0.8;
        let k = b.words.len();
        for (j, w) in b.words.iter().enumerate() {
            let t = if k == 1 { 0.0 } else { j as f32 / (k - 1) as f32 - 0.5 };
            let wa = a + t * spread;
            let (wx, wy) = (bx + WORD_R * wa.cos(), by + WORD_R * wa.sin());
            node(&mut items, &w.word, Some(w.gloss_en.clone()), Some(&branch), 2, wx, wy);
        }
    }
    items
}

/// The words of a map as plain JSON rows, e.g. to add them to Review.
pub fn words(map: &MindMap) -> Vec<Value> {
    map.branches
        .iter()
        .flat_map(|b| {
            b.words.iter().map(move |w| json!({"word": w.word, "branch": b.label, "gloss_en": w.gloss_en}))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiOutput;

    fn pasar() -> MindMap {
        let w = |word: &str, en: &str, ar: &str| MapWord {
            word: word.into(),
            gloss_en: en.into(),
            gloss_ar: ar.into(),
        };
        MindMap {
            center: "di pasar".into(),
            branches: vec![
                Branch {
                    label: "orang".into(),
                    words: vec![w("penjual", "seller", "بائع"), w("pembeli", "buyer", "مشترٍ")],
                },
                Branch {
                    label: "barang".into(),
                    words: vec![
                        w("sayur", "vegetables", "خضار"),
                        w("ikan", "fish", "سمك"),
                        w("buah", "fruit", "فاكهة"),
                    ],
                },
                Branch {
                    label: "harga".into(),
                    words: vec![w("murah", "cheap", "رخيص"), w("mahal", "expensive", "غالٍ")],
                },
            ],
        }
    }

    #[test]
    fn validation_follows_the_golden_rubric() {
        let m = pasar();
        assert!(m.validate().is_ok());
        assert!(!m.needs_review());

        let mut two = m.clone();
        two.branches.pop();
        assert!(two.check(&json!({})).unwrap_err().contains("3-6 branches"));

        let mut dup = m.clone();
        dup.branches[2].words[0].word = "Ikan".into();
        assert!(dup.validate().unwrap_err().contains("twice"));

        let mut latin = m.clone();
        latin.branches[0].words[0].gloss_ar = "baa'i".into();
        assert!(latin.validate().unwrap_err().contains("Arabic script"));

        let mut lonely = m;
        lonely.branches[0].words.truncate(1);
        assert!(lonely.validate().is_err());
    }

    #[test]
    fn layout_places_every_word_around_its_branch() {
        let m = pasar();
        let mut n = 0;
        let items = layout(&m, "doc", 0.0, 0.0, || {
            n += 1;
            format!("i{n}")
        });
        let nodes: Vec<_> = items.iter().filter(|i| matches!(i.kind, ItemKind::MapNode { .. })).collect();
        let arrows = items.len() - nodes.len();
        assert_eq!(nodes.len(), 1 + 3 + 7);
        assert_eq!(arrows, nodes.len() - 1, "every node but the center hangs on an arrow");
        // The first branch sits straight above the center.
        let first = nodes[1];
        assert!((first.x + NODE_W / 2.0).abs() < 0.01 && (first.y + NODE_H / 2.0 + BRANCH_R).abs() < 0.01);
        // Words land further from the center than their branch.
        for item in &nodes {
            if let ItemKind::MapNode { depth: 2, .. } = item.kind {
                let (x, y) = (item.x + NODE_W / 2.0, item.y + NODE_H / 2.0);
                assert!((x * x + y * y).sqrt() > BRANCH_R);
            }
        }
        assert_eq!(words(&m).len(), 7);
        let req = request(" di pasar ", "bipa1", &["pasar".into()], Lang::Id);
        assert_eq!(req.input["topic"], "di pasar");
        assert!(!req.personal);
    }
}
