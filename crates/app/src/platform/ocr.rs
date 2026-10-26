//! Engine-neutral OCR helpers: pure functions shared by the Android (ML Kit)
//! and macOS (Apple Vision) bridges, so they can be tested on any host.

use kabar_protocol::reader::{OcrBlock, OcrLine, OcrResult, Rect};

/// Parses the JSON an engine bridge produced (Android's `OcrBridge.kt`) and tidies it.
pub fn parse_result_json(json: &str) -> Result<OcrResult, String> {
    let raw: OcrResult =
        serde_json::from_str(json).map_err(|e| format!("unexpected OCR result shape: {e}"))?;
    Ok(tidy(raw))
}

/// Drops blank lines and empty blocks, trims text, clamps confidence to 0..1 and
/// gives blocks without a usable box the union of their lines' boxes.
pub fn tidy(mut result: OcrResult) -> OcrResult {
    result.blocks = std::mem::take(&mut result.blocks)
        .into_iter()
        .filter_map(|mut block| {
            block.lines = std::mem::take(&mut block.lines)
                .into_iter()
                .filter_map(|mut line| {
                    let trimmed = line.text.trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    if trimmed.len() != line.text.len() {
                        line.text = trimmed.to_string();
                    }
                    line.confidence = line.confidence.filter(|c| c.is_finite()).map(|c| c.clamp(0.0, 1.0));
                    Some(line)
                })
                .collect();
            if block.lines.is_empty() {
                return None;
            }
            if block.bbox.w <= 0.0 || block.bbox.h <= 0.0 {
                block.bbox = union_all(block.lines.iter().map(|l| l.bbox));
            }
            Some(block)
        })
        .collect();
    result
}

/// Converts a normalized rectangle with its origin at the bottom-left (Apple
/// Vision) into pixels with the origin at the top-left.
pub fn normalized_bottom_left_to_pixels(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    image_width: f64,
    image_height: f64,
) -> Rect {
    let px = |v: f64| v as f32;
    Rect {
        x: px(x * image_width),
        y: px((1.0 - y - h) * image_height),
        w: px(w * image_width),
        h: px(h * image_height),
    }
}

/// Groups single lines (engines like Apple Vision report no blocks) into
/// paragraph-like blocks, in reading order: top to bottom, then left to right.
/// A line joins the block above it when it starts within about one line height
/// below it and the two overlap horizontally, which keeps chat bubbles and
/// their timestamps together but separates bubbles from each other.
pub fn group_lines(mut lines: Vec<OcrLine>) -> Vec<OcrBlock> {
    lines.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y).then(a.bbox.x.total_cmp(&b.bbox.x)));
    let mut blocks: Vec<OcrBlock> = Vec::new();
    for line in lines {
        let joins = blocks.iter().rposition(|block| {
            let last = block.lines.last().map(|l| l.bbox).unwrap_or(block.bbox);
            let gap = line.bbox.y - last.bottom();
            let line_height = last.h.max(line.bbox.h).max(1.0);
            let overlap = line.bbox.right().min(block.bbox.right()) - line.bbox.x.max(block.bbox.x);
            gap < line_height * 0.8 && gap > -line_height * 0.5 && overlap > 0.0
        });
        match joins {
            Some(i) => {
                let block = &mut blocks[i];
                block.bbox = union(block.bbox, line.bbox);
                block.lines.push(line);
            }
            None => blocks.push(OcrBlock { bbox: line.bbox, lines: vec![line] }),
        }
    }
    blocks
}

fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect { x, y, w: a.right().max(b.right()) - x, h: a.bottom().max(b.bottom()) - y }
}

fn union_all(rects: impl Iterator<Item = Rect>) -> Rect {
    rects.reduce(union).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabar_protocol::reader::OcrEngine;

    const FIXTURE: &str = include_str!("../../../../fixtures/ocr/whatsapp_javanese.ml_kit.json");

    fn line(text: &str, x: f32, y: f32, w: f32, h: f32) -> OcrLine {
        OcrLine { text: text.into(), bbox: Rect { x, y, w, h }, confidence: Some(0.9) }
    }

    #[test]
    fn parses_kotlin_bridge_output() {
        let r = parse_result_json(FIXTURE).unwrap();
        assert_eq!(r.engine, OcrEngine::MlKit);
        assert_eq!((r.width, r.height), (1440.0, 3120.0));
        assert_eq!(r.blocks.len(), 4, "the empty block is dropped");
        assert_eq!(r.plain_text(), "Sek tak lapor pusat\n\nng prabowo?\n12:41\n\npiye dhe\n\narp dicul piro");
        assert_eq!(r.blocks[2].lines[0].confidence, None);
        assert_eq!(r.blocks[1].lines[1].bbox, Rect { x: 352.0, y: 603.0, w: 132.0, h: 46.0 });
    }

    #[test]
    fn empty_block_box_becomes_union_of_lines() {
        let r = parse_result_json(FIXTURE).unwrap();
        assert_eq!(r.blocks[3].lines.len(), 1, "whitespace-only line is dropped");
        assert_eq!(r.blocks[3].bbox, Rect { x: 930.0, y: 861.0, w: 444.0, h: 62.0 });
    }

    #[test]
    fn rejects_wrong_shape() {
        assert!(parse_result_json(r#"{"engine":"ml_kit"}"#).is_err());
        assert!(parse_result_json(r#"{"engine":"paddle","width":1,"height":1,"blocks":[]}"#).is_err());
    }

    #[test]
    fn tidy_trims_and_clamps() {
        let mut l = line("  Apa kabar?  ", 0.0, 0.0, 10.0, 10.0);
        l.confidence = Some(1.7);
        let r = tidy(OcrResult {
            engine: OcrEngine::AppleVision,
            width: 10.0,
            height: 10.0,
            blocks: vec![OcrBlock { bbox: Rect::default(), lines: vec![l] }],
        });
        assert_eq!(r.blocks[0].lines[0].text, "Apa kabar?");
        assert_eq!(r.blocks[0].lines[0].confidence, Some(1.0));
    }

    #[test]
    fn vision_box_to_top_left_pixels() {
        // A line in the top-left quarter of a 1000x2000 image.
        let r = normalized_bottom_left_to_pixels(0.1, 0.9, 0.5, 0.05, 1000.0, 2000.0);
        assert!((r.x - 100.0).abs() < 1e-3);
        assert!((r.y - 100.0).abs() < 1e-3, "y = {}", r.y);
        assert!((r.w - 500.0).abs() < 1e-3);
        assert!((r.h - 100.0).abs() < 1e-3);
        // The full image maps to the full image.
        let full = normalized_bottom_left_to_pixels(0.0, 0.0, 1.0, 1.0, 1440.0, 3120.0);
        assert_eq!(full, Rect { x: 0.0, y: 0.0, w: 1440.0, h: 3120.0 });
    }

    #[test]
    fn groups_chat_bubbles() {
        // Out of order on purpose: engines don't promise reading order.
        let lines = vec![
            line("piye dhe", 1012.0, 742.0, 281.0, 58.0),
            line("12:41", 352.0, 603.0, 132.0, 46.0),
            line("Sek tak lapor pusat", 96.0, 402.0, 612.0, 61.0),
            line("ng prabowo?", 96.0, 521.0, 388.0, 60.0),
            line("arp dicul piro", 930.0, 861.0, 444.0, 62.0),
        ];
        let blocks = group_lines(lines);
        let texts: Vec<Vec<&str>> =
            blocks.iter().map(|b| b.lines.iter().map(|l| l.text.as_str()).collect()).collect();
        assert_eq!(
            texts,
            vec![
                vec!["Sek tak lapor pusat"],
                vec!["ng prabowo?", "12:41"],
                vec!["piye dhe"],
                vec!["arp dicul piro"],
            ]
        );
        assert_eq!(blocks[1].bbox, Rect { x: 96.0, y: 521.0, w: 388.0, h: 128.0 });
    }

    #[test]
    fn groups_a_paragraph() {
        let lines = vec![
            line("iphone 17 pm 512gb resmi ibox", 40.0, 100.0, 900.0, 40.0),
            line("mulus nomin garansi jalan", 40.0, 150.0, 760.0, 40.0),
            line("minat wa", 40.0, 200.0, 240.0, 40.0),
        ];
        let blocks = group_lines(lines);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].bbox, Rect { x: 40.0, y: 100.0, w: 900.0, h: 140.0 });
    }
}
