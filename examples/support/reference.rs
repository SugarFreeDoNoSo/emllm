//! Referencia convencional exclusiva de pruebas y demostración, fuera del motor.

pub const INPUTS: [[f64; 2]; 3] = [[0.0, 0.0], [0.5, -0.5], [-0.5, 0.5]];

pub fn infer([x, y]: [f64; 2]) -> f64 {
    let first = 0.5 * x - 0.5 * y;
    let second = 0.5 * y - 0.5;
    let hidden_first = (-first * first).exp();
    let hidden_second = (-second * second).exp();
    0.5 * hidden_first - 0.5 * hidden_second
}
