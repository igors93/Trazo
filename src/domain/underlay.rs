//! External image and PDF underlays with two-point scale calibration,
//! opacity control and lock state (#40).

use serde::{Deserialize, Serialize};

use super::{geometry::Point, model::Project};

fn default_opacity() -> f32 {
    0.45
}

fn default_true() -> bool {
    true
}

fn default_page() -> u32 {
    1
}

/// An external raster image or rasterized PDF page placed behind the floor plan
/// as a tracing reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageUnderlay {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub source_path: String,
    #[serde(default = "default_page")]
    pub page: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// Top-left corner in world coordinates (metres).
    pub origin: Point,
    /// Width of the image in world metres; height follows the pixel aspect ratio.
    pub width_m: f64,
    #[serde(default)]
    pub rotation_deg: f64,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    /// Optional PNG-encoded preview cached inside the project file so the
    /// underlay remains visible even when the original file moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_png_base64: Option<String>,
}

impl ImageUnderlay {
    /// World height in metres, preserving the original image aspect ratio.
    pub fn height_m(&self) -> f64 {
        if self.pixel_width == 0 || self.pixel_height == 0 {
            return self.width_m;
        }
        self.width_m * (f64::from(self.pixel_height) / f64::from(self.pixel_width))
    }

    /// Axis-aligned bounding box `(min, max)` in world metres.
    pub fn bounds(&self) -> (Point, Point) {
        let h = self.height_m();
        let corners = [
            self.origin,
            Point::new(self.origin.x + self.width_m, self.origin.y),
            Point::new(self.origin.x + self.width_m, self.origin.y + h),
            Point::new(self.origin.x, self.origin.y + h),
        ];
        if self.rotation_deg.abs() < 1e-6 {
            return (corners[0], corners[2]);
        }
        let rad = self.rotation_deg.to_radians();
        let (sin, cos) = rad.sin_cos();
        let center = Point::new(self.origin.x + self.width_m * 0.5, self.origin.y + h * 0.5);
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for c in corners {
            let dx = c.x - center.x;
            let dy = c.y - center.y;
            let rx = center.x + dx * cos - dy * sin;
            let ry = center.y + dx * sin + dy * cos;
            min_x = min_x.min(rx);
            min_y = min_y.min(ry);
            max_x = max_x.max(rx);
            max_y = max_y.max(ry);
        }
        (Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    /// Calibrates the underlay's real-world scale using two picked points `p1`
    /// and `p2` whose real-world distance should equal `real_distance_m`.
    /// Keeps `p1` anchored in world space so alignment is preserved.
    pub fn calibrate_two_points(
        &mut self,
        p1: Point,
        p2: Point,
        real_distance_m: f64,
    ) -> Result<f64, String> {
        if self.locked {
            return Err(
                "A imagem de fundo está bloqueada. Desbloqueie antes de calibrar.".to_owned(),
            );
        }
        if !real_distance_m.is_finite() || real_distance_m < 0.05 || real_distance_m > 10_000.0 {
            return Err("Informe uma distância real entre 0,05 m e 10.000 m.".to_owned());
        }
        let current = p1.distance(p2);
        if !current.is_finite() || current < 1e-4 {
            return Err("Os dois pontos de calibração precisam estar separados.".to_owned());
        }
        let factor = real_distance_m / current;
        let new_width = self.width_m * factor;
        if !new_width.is_finite() || !(0.1..=50_000.0).contains(&new_width) {
            return Err("A escala resultante ficou fora dos limites suportados.".to_owned());
        }
        self.width_m = new_width;
        self.origin = Point::new(
            p1.x + (self.origin.x - p1.x) * factor,
            p1.y + (self.origin.y - p1.y) * factor,
        )
        .rounded();
        Ok(factor)
    }
}

impl Project {
    /// Adds an external image/PDF underlay behind the floor plan and returns its ID.
    #[allow(clippy::too_many_arguments)]
    pub fn add_underlay(
        &mut self,
        name: String,
        source_path: String,
        page: u32,
        pixel_width: u32,
        pixel_height: u32,
        origin: Point,
        width_m: f64,
        embedded_png_base64: Option<String>,
    ) -> u64 {
        let id = self.underlays.iter().map(|u| u.id).max().unwrap_or(0) + 1;
        self.underlays.push(ImageUnderlay {
            id,
            name: if name.trim().is_empty() {
                "Imagem de fundo".to_owned()
            } else {
                name.trim().to_owned()
            },
            source_path,
            page: page.max(1),
            pixel_width: pixel_width.max(1),
            pixel_height: pixel_height.max(1),
            origin: origin.rounded(),
            width_m: width_m.clamp(0.2, 50_000.0),
            rotation_deg: 0.0,
            opacity: default_opacity(),
            visible: true,
            locked: false,
            embedded_png_base64,
        });
        id
    }

    pub fn remove_underlay(&mut self, id: u64) -> Result<(), String> {
        let Some(pos) = self.underlays.iter().position(|u| u.id == id) else {
            return Err("Imagem de fundo não encontrada.".to_owned());
        };
        if self.underlays[pos].locked {
            return Err("Desbloqueie a imagem de fundo antes de removê-la.".to_owned());
        }
        self.underlays.remove(pos);
        Ok(())
    }

    pub fn calibrate_underlay(
        &mut self,
        id: u64,
        p1: Point,
        p2: Point,
        real_distance_m: f64,
    ) -> Result<f64, String> {
        let underlay = self
            .underlays
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or_else(|| "Imagem de fundo não encontrada.".to_owned())?;
        underlay.calibrate_two_points(p1, p2, real_distance_m)
    }

    #[allow(dead_code)]
    pub fn move_underlay(&mut self, id: u64, delta: Point) -> Result<(), String> {
        let underlay = self
            .underlays
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or_else(|| "Imagem de fundo não encontrada.".to_owned())?;
        if underlay.locked {
            return Err("A imagem de fundo está bloqueada.".to_owned());
        }
        underlay.origin = underlay.origin.translated(delta).rounded();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_point_calibration_scales_accurately_and_anchors_first_point() {
        let mut project = Project::default();
        let id = project.add_underlay(
            "Planta escaneada".to_owned(),
            "planta.png".to_owned(),
            1,
            1000,
            500,
            Point::new(0.0, 0.0),
            10.0,
            None,
        );
        // Two points currently 2.0 m apart represent a 5.0 m wall -> 2.5× scale.
        let p1 = Point::new(2.0, 1.0);
        let p2 = Point::new(4.0, 1.0);
        let factor = project.calibrate_underlay(id, p1, p2, 5.0).unwrap();
        assert!((factor - 2.5).abs() < 1e-6);

        let u = project.underlays.iter().find(|u| u.id == id).unwrap();
        assert!((u.width_m - 25.0).abs() < 1e-6);
        assert!((u.height_m() - 12.5).abs() < 1e-6);
        // Origin scaled around p1 = (2, 1): (2 + (0 - 2)*2.5, 1 + (0 - 1)*2.5) = (-3, -1.5).
        assert!((u.origin.x - (-3.0)).abs() < 1e-4);
        assert!((u.origin.y - (-1.5)).abs() < 1e-4);
    }

    #[test]
    fn locked_underlay_rejects_calibration_move_and_removal() {
        let mut project = Project::default();
        let id = project.add_underlay(
            "PDF".to_owned(),
            "planta.pdf".to_owned(),
            1,
            800,
            600,
            Point::new(0.0, 0.0),
            12.0,
            None,
        );
        project.underlays[0].locked = true;
        assert!(
            project
                .calibrate_underlay(id, Point::new(0.0, 0.0), Point::new(1.0, 0.0), 3.0)
                .is_err()
        );
        assert!(project.move_underlay(id, Point::new(1.0, 0.0)).is_err());
        assert!(project.remove_underlay(id).is_err());

        project.underlays[0].locked = false;
        project.move_underlay(id, Point::new(1.5, -0.5)).unwrap();
        assert!((project.underlays[0].origin.x - 1.5).abs() < 1e-4);
        project.remove_underlay(id).unwrap();
        assert!(project.underlays.is_empty());
    }

    #[test]
    fn underlay_round_trips_through_project_json() {
        let mut project = Project::default();
        project.add_underlay(
            "Croqui".to_owned(),
            "croqui.jpg".to_owned(),
            1,
            640,
            480,
            Point::new(1.0, 2.0),
            8.0,
            Some("AAAA".to_owned()),
        );
        let json = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.underlays.len(), 1);
        assert_eq!(loaded.underlays[0].name, "Croqui");
        assert_eq!(
            loaded.underlays[0].embedded_png_base64.as_deref(),
            Some("AAAA")
        );
    }
}
