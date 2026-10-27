//! Map projection and the places on the route.
//!
//! One equirectangular projection with a cos(lat) correction at 2°S, shared by
//! the build-time map generator (`kabar-tools`, `journey-map`) and the app, so
//! the drawn coastlines and the route pins always line up.

/// The map covers Indonesia: 94–142°E, 11°S–7°N.
pub const LON_MIN: f64 = 94.0;
pub const LON_MAX: f64 = 142.0;
pub const LAT_MIN: f64 = -11.0;
pub const LAT_MAX: f64 = 7.0;
/// Map units per degree of latitude.
pub const SCALE: f64 = 20.0;
/// cos(2°): longitude shrink at the archipelago's middle latitude.
const COS_REF: f64 = 0.999_390_827_019_095_7;

/// Longitude/latitude to map units (x east, y south).
pub fn project(lon: f64, lat: f64) -> (f64, f64) {
    ((lon - LON_MIN) * SCALE * COS_REF, (LAT_MAX - lat) * SCALE)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl ViewBox {
    fn from_bounds(lon_min: f64, lat_min: f64, lon_max: f64, lat_max: f64) -> Self {
        let (x0, y0) = project(lon_min, lat_max);
        let (x1, y1) = project(lon_max, lat_min);
        Self { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }

    /// `p` moved inside the box, at least `margin` from every edge (or to
    /// the middle, on an axis narrower than two margins).
    pub fn clamp(&self, p: (f64, f64), margin: f64) -> (f64, f64) {
        let axis = |v: f64, lo: f64, len: f64| {
            if len <= 2.0 * margin { lo + len / 2.0 } else { v.clamp(lo + margin, lo + len - margin) }
        };
        (axis(p.0, self.x, self.w), axis(p.1, self.y, self.h))
    }

    /// The SVG `viewBox` attribute.
    pub fn attr(&self) -> String {
        format!("{:.1} {:.1} {:.1} {:.1}", self.x, self.y, self.w, self.h)
    }
}

/// The whole archipelago, for the BIPA islands.
pub fn indonesia_view() -> ViewBox {
    ViewBox::from_bounds(LON_MIN, LAT_MIN, LON_MAX, LAT_MAX)
}

/// Central and East Java, for the Javanese train line.
pub fn java_view() -> ViewBox {
    ViewBox::from_bounds(109.55, -8.45, 113.35, -6.55)
}

/// The region the generator draws in detail for [`java_view`] (all of Java,
/// so panning never runs off the land).
pub const JAVA_DETAIL: (f64, f64, f64, f64) = (104.5, -9.2, 116.0, -5.2);

/// A level's place on the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// The curriculum level id it belongs to.
    pub level: &'static str,
    pub name: &'static str,
    /// Real approximate location.
    pub lon: f64,
    pub lat: f64,
    /// Where the pin is drawn; differs from the real location only where two
    /// pins would overlap (Kotagede sits 4 km from central Yogyakarta).
    pub map_lon: f64,
    pub map_lat: f64,
}

const fn place(level: &'static str, name: &'static str, lon: f64, lat: f64) -> Place {
    Place { level, name, lon, lat, map_lon: lon, map_lat: lat }
}

/// Every place on both routes (SPEC §10), with real approximate coordinates.
pub const PLACES: &[Place] = &[
    // BIPA: island hopping.
    place("bipa1", "Jakarta", 106.85, -6.21),
    place("bipa2", "Bali", 115.19, -8.41),
    place("bipa3", "Lombok", 116.32, -8.62),
    // Tana Toraja, in the heart of South Sulawesi's highlands.
    place("bipa4", "Sulawesi", 119.87, -3.05),
    // Central Kalimantan, north of Palangka Raya.
    place("bipa5", "Kalimantan", 113.92, -1.55),
    // Bukittinggi, West Sumatra (Minangkabau country).
    place("bipa6", "Sumatra", 100.37, -0.31),
    place("bipa7", "Raja Ampat", 130.62, -0.45),
    // Javanese: Yogyakarta and Madiun only (Mataraman Javanese), there and
    // back along the Jogja–Madiun line. Jogja's four levels sit within a few
    // km of each other, so their pins are nudged apart around the Kraton:
    // Malioboro north, Pakualaman west, Kotagede south-east.
    Place { level: "j1", name: "Malioboro", lon: 110.365, lat: -7.79, map_lon: 110.30, map_lat: -7.65 },
    place("j2", "Madiun", 111.52, -7.63),
    Place { level: "j3", name: "Pakualaman", lon: 110.375, lat: -7.80, map_lon: 110.20, map_lat: -7.86 },
    place("j4", "Yogyakarta", 110.364, -7.805),
    // Madiun again, back for the advanced level: drawn south-east of J2's pin.
    Place { level: "j5", name: "Kutha Pendekar", lon: 111.52, lat: -7.63, map_lon: 111.62, map_lat: -7.78 },
    Place { level: "j6", name: "Kotagede", lon: 110.40, lat: -7.83, map_lon: 110.52, map_lat: -7.98 },
];

/// The place for a level: by level id, else by the start of its place name
/// ("Malioboro, Yogyakarta", "Raja Ampat, Papua").
pub fn place_for(level_id: &str, place_name: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.level == level_id).or_else(|| {
        let name = place_name.to_lowercase();
        PLACES.iter().find(|p| name.starts_with(&p.name.to_lowercase()))
    })
}

impl Place {
    /// The pin position in map units.
    pub fn point(&self) -> (f64, f64) {
        project(self.map_lon, self.map_lat)
    }
}
