//! Generates the Journey Map's base maps from Natural Earth (public domain).
//!
//! ```sh
//! curl -sSfLo /tmp/ne/countries.geojson \
//!   https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_admin_0_countries.geojson
//! cargo run -p kabar-tools --bin journey-map -- /tmp/ne/countries.geojson crates/app/assets/journey
//! ```
//!
//! Reads the 1:10m Admin 0 countries layer, clips it to Indonesia
//! (94–142°E, 11°S–7°N), projects with `kabar_core::journey::geo::project`
//! (equirectangular, cos-lat corrected), simplifies with Douglas–Peucker and
//! writes two SVGs into the output directory:
//!
//! - `indonesia.svg`: the whole archipelago, for the BIPA islands;
//! - `java.svg`: Java in finer detail, for the Javanese train line.
//!
//! Each has `path.land-id` (Indonesia) and `path.land-other` (neighbours) and a
//! `g.places` layer with every route place for checking by eye. Don't commit the
//! GeoJSON; commit the SVGs (and list Natural Earth in ATTRIBUTION.md).

use std::path::PathBuf;
use std::process::ExitCode;

use kabar_core::journey::geo::{self, JAVA_DETAIL, PLACES, ViewBox};
use kabar_tools::mapgen::{Bounds, Pt, path_data, prepare};
use serde_json::Value;

struct Output {
    file: &'static str,
    title: &'static str,
    bounds: Bounds,
    view: ViewBox,
    /// Simplification tolerance and minimum island area, in map units.
    tol: f64,
    min_area: f64,
    decimals: usize,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, out_dir] = args.as_slice() else {
        eprintln!("usage: journey-map <ne_10m_admin_0_countries.geojson> <out-dir>");
        return ExitCode::from(2);
    };
    match run(input, &PathBuf::from(out_dir)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("journey-map: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &str, out_dir: &std::path::Path) -> Result<(), String> {
    let text = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| format!("{input}: {e}"))?;
    let features = json["features"].as_array().ok_or("no features array")?;

    // (is Indonesia, ring in lon/lat)
    let mut rings: Vec<(bool, Vec<Pt>)> = Vec::new();
    for f in features {
        let idn = f["properties"]["ADM0_A3"].as_str() == Some("IDN");
        let g = &f["geometry"];
        let polys: Vec<&Value> = match g["type"].as_str() {
            Some("Polygon") => vec![&g["coordinates"]],
            Some("MultiPolygon") => {
                g["coordinates"].as_array().map(|a| a.iter().collect()).unwrap_or_default()
            }
            _ => continue,
        };
        for poly in polys {
            for ring in poly.as_array().into_iter().flatten() {
                let pts: Vec<Pt> = ring
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?)))
                    .collect();
                if pts.len() >= 4 {
                    rings.push((idn, pts));
                }
            }
        }
    }
    if !rings.iter().any(|(idn, _)| *idn) {
        return Err("no Indonesia (ADM0_A3 = IDN) in the input; is this the Admin 0 countries layer?".into());
    }

    let outputs = [
        Output {
            file: "indonesia.svg",
            title: "Indonesia",
            // A little past the view so clipped edges never show.
            bounds: (geo::LON_MIN - 1.0, geo::LAT_MIN - 1.0, geo::LON_MAX + 1.0, geo::LAT_MAX + 1.0),
            view: geo::indonesia_view(),
            tol: 0.35,
            min_area: 0.5,
            decimals: 1,
        },
        Output {
            file: "java.svg",
            title: "Java",
            bounds: JAVA_DETAIL,
            view: geo::java_view(),
            tol: 0.04,
            min_area: 0.01,
            decimals: 2,
        },
    ];

    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    for o in &outputs {
        let mut id = Vec::new();
        let mut other = Vec::new();
        for (idn, ring) in &rings {
            if let Some(r) = prepare(ring, o.bounds, o.tol, o.min_area) {
                if *idn { id.push(r) } else { other.push(r) }
            }
        }
        let svg = render(o, &path_data(&id, o.decimals), &path_data(&other, o.decimals));
        let path = out_dir.join(o.file);
        std::fs::write(&path, &svg).map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "{}: {} Indonesian + {} neighbouring rings, {} KB",
            path.display(),
            id.len(),
            other.len(),
            svg.len() / 1024
        );
    }
    println!("\nplaces (real lon/lat → map units):");
    for p in PLACES {
        let (x, y) = p.point();
        println!("  {:<6} {:<11} {:>7.2} {:>6.2} → {x:>6.1} {y:>6.1}", p.level, p.name, p.lon, p.lat);
    }
    Ok(())
}

fn render(o: &Output, id: &str, other: &str) -> String {
    let mut places = String::new();
    let r = o.view.w / 300.0;
    for p in PLACES {
        let (x, y) = p.point();
        places.push_str(&format!(
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\"><title>{} ({}): {:.2}E {:.2}S</title></circle>",
            p.name, p.level, p.lon, -p.lat
        ));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view}\" role=\"img\" aria-label=\"{title}\">\
<!-- Generated by the kabar-tools journey-map tool; do not edit. \
Made with Natural Earth (public domain), simplified and restyled for Kabar. -->\
<rect class=\"sea\" x=\"{vx:.1}\" y=\"{vy:.1}\" width=\"{vw:.1}\" height=\"{vh:.1}\" fill=\"#dfe8ea\"/>\
<path class=\"land-other\" fill=\"#e6e0d4\" fill-rule=\"evenodd\" d=\"{other}\"/>\
<path class=\"land-id\" fill=\"#cfdcb9\" fill-rule=\"evenodd\" d=\"{id}\"/>\
<g class=\"places\" fill=\"#c8573a\">{places}</g></svg>\n",
        view = o.view.attr(),
        title = o.title,
        vx = o.view.x,
        vy = o.view.y,
        vw = o.view.w,
        vh = o.view.h,
    )
}
