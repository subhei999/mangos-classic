use serde::{Deserialize, Serialize};

/// A position in the game world, including map context and facing direction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPosition {
    pub map_id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

impl WorldPosition {
    /// Create a new world position.
    pub fn new(map_id: u32, x: f32, y: f32, z: f32, orientation: f32) -> Self {
        Self {
            map_id,
            x,
            y,
            z,
            orientation,
        }
    }

    /// Calculate the 3D Euclidean distance to another position.
    ///
    /// Returns `f32::MAX` if the positions are on different maps.
    pub fn distance_to(&self, other: &WorldPosition) -> f32 {
        if self.map_id != other.map_id {
            return f32::MAX;
        }
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Calculate the 2D (XY-plane) Euclidean distance to another position.
    ///
    /// Returns `f32::MAX` if the positions are on different maps.
    pub fn distance_2d(&self, other: &WorldPosition) -> f32 {
        if self.map_id != other.map_id {
            return f32::MAX;
        }
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl Default for WorldPosition {
    fn default() -> Self {
        Self {
            map_id: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            orientation: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_same_point() {
        let pos = WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0);
        assert!((pos.distance_to(&pos) - 0.0).abs() < f32::EPSILON);
        assert!((pos.distance_2d(&pos) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn distance_3d() {
        let a = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
        let b = WorldPosition::new(0, 3.0, 4.0, 0.0, 0.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn distance_2d_ignores_z() {
        let a = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
        let b = WorldPosition::new(0, 3.0, 4.0, 100.0, 0.0);
        assert!((a.distance_2d(&b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn distance_different_maps() {
        let a = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
        let b = WorldPosition::new(1, 1.0, 1.0, 1.0, 0.0);
        assert_eq!(a.distance_to(&b), f32::MAX);
        assert_eq!(a.distance_2d(&b), f32::MAX);
    }
}
