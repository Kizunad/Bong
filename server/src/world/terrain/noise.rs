#[allow(dead_code)]
pub fn coherent_noise_2d(world_x: f64, world_z: f64, scale: f64, seed: i32) -> f64 {
    let scale = scale.max(1.0);
    let sx = world_x / scale;
    let sz = world_z / scale;
    let sp = seed as f64 * 0.017;

    (sx * 1.17 + sz * 0.83 + sp).sin() * 0.5
        + (sx * -0.71 + sz * 1.29 - sp * 1.3).cos() * 0.3
        + (sx * 2.03 - sz * 1.61 + sp * 0.7).sin() * 0.2
}
