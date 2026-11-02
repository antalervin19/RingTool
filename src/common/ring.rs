pub const C: f32 = 230.0;
pub const R1 : f32 = 46.0;
pub const R2 : f32 = 108.0;
pub const R3 : f32 = 114.0;
pub const R4 : f32 = 205.0;

#[derive(Clone, Copy, Debug)]
pub struct Ring {
    pub r1: f32,
    pub r2: f32,
    pub first: f32,
    pub step: f32,
    pub count: usize,
}

impl Ring {
    pub fn center_angle(&self, i: usize) -> f32 {
        self.first + i as f32 * self.step
    }

    pub fn hit(&self, dx: f32, dy: f32) -> Option<usize> {
        let r = (dx * dx + dy * dy).sqrt();

        if r < self.r1 || r > self.r2 {
            return None;
        }

        let angle = dx.atan2(-dy).to_degrees();
        let rel = (angle - (self.first - self.step / 2.0)).rem_euclid(360.0);
        let i = (rel / self.step) as usize;

        (i < self.count).then_some(i)
    }
}

pub fn polar(r: f32, deg: f32) -> (f32, f32) {
    let a = deg.to_radians();
    (C + r * a.sin(), C - r * a.cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_the_segment_you_aim_at() {
        let ring = Ring { r1: R3, r2: R4, first: 186.0, step: 30.0, count: 4 };
        for i in 0..4 {
            let (x, y) = polar(150.0, ring.center_angle(i));
            assert_eq!(ring.hit(x - C, y - C), Some(i));
        }
    }

    #[test]
    fn misses_outside_the_band_and_the_arc() {
        let ring = Ring { r1: R3, r2: R4, first: 186.0, step: 30.0, count: 4 };
        assert_eq!(ring.hit(0.0, 0.0), None);
        let (x, y) = polar(150.0, 0.0);
        assert_eq!(ring.hit(x - C, y - C), None);
    }
}