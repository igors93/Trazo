//! MCP tools covering Trazo's quantities takeoff, ergonomics verification,
//! CAD layers, annotations, associative dimensions, vertical circulation,
//! technical views (sections/elevations/details) and ABNT sheet layout.

mod circulation;
mod dimensions;
mod layers;
mod notes;
mod parse;
mod quantities;
mod sheet;
#[cfg(test)]
mod tests;
mod views;

use parse::{line_type_key, parse_elevation_side, parse_line_type, round2};
pub(super) use parse::{parse_hex_color, parse_railing_side, parse_scale, parse_stair_shape};
