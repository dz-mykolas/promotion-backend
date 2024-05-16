use chrono::Datelike;

use geo::{Coord, Point};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

#[derive(Debug, Clone)]
pub struct Location {
    pub name: &'static str,
    pub coordinates: Point,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.coordinates.x(), self.coordinates.y())
    }
}

// Function to pick a random location based on current day.
pub fn pick_location_by_day() -> Location {
    let year = chrono::Utc::now().year() as u8;
    let month = chrono::Utc::now().month() as u8;
    let day = chrono::Utc::now().weekday().num_days_from_monday() as u8;

    let seed_num = year + month + day;
    let seed = [seed_num; 32];
    let mut rng = StdRng::from_seed(seed);

    let index = rng.gen_range(0..LOCATIONS.len());

    LOCATIONS[index].clone()
}

// 10 location constants with coordinates
const LOCATIONS: [Location; 10] = [
    Location {
        name: "New York",
        coordinates: Point { 0: Coord { x: -74.006, y: 40.7128 } },
    },
    Location {
        name: "Los Angeles",
        coordinates: Point { 0: Coord { x: -118.2437, y: 34.0522 } },
    },
    Location {
        name: "Chicago",
        coordinates: Point { 0: Coord { x: -87.6298, y: 41.8781 } },
    },
    Location {
        name: "Houston",
        coordinates: Point { 0: Coord { x: -95.3698, y: 29.7604 } },
    },
    Location {
        name: "Phoenix",
        coordinates: Point { 0: Coord { x: -112.074, y: 33.4484 } },
    },
    Location {
        name: "Philadelphia",
        coordinates: Point { 0: Coord { x: -75.1652, y: 39.9526 } },
    },
    Location {
        name: "San Antonio",
        coordinates: Point { 0: Coord { x: -98.4936, y: 29.4241 } },
    },
    Location {
        name: "San Diego",
        coordinates: Point { 0: Coord { x: -117.1611, y: 32.7157 } },
    },
    Location {
        name: "Dallas",
        coordinates: Point { 0: Coord { x: -96.7969, y: 32.7767 } },
    },
    Location {
        name: "San Jose",
        coordinates: Point { 0: Coord { x: -121.8863, y: 37.3382 } },
    },
];
