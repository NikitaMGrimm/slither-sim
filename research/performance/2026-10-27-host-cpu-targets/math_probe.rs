// Supplemental finite-input bit oracle for the crate's floating math operations.
// This samples code generation; the twelve full traces are the gameplay gate.
use std::hint::black_box;

fn main() {
    let mut values = vec![
        -1e300_f64, -1_048_576., -1500., -1., -0., 0., f64::from_bits(1),
        f64::MIN_POSITIVE, 0.1, 1., 2.25, 1500., 1_048_576., 1e300,
    ];
    for x in [-1500_f64, -1., -0., 0., 1., 1500.] {
        values.extend([x.next_down(), x.next_up()]);
    }
    let mut state = 7_u64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        values.push((state as i64 as f64 / i64::MAX as f64) * 1_048_576.);
    }
    for value in values {
        let x = black_box(value);
        let positive = black_box(x.abs().max(f64::MIN_POSITIVE));
        let y = black_box(0.37_f64);
        let (s, c) = x.sin_cos();
        for result in [
            x.sin(), x.cos(), s, c, x.atan2(y), x.hypot(y), positive.sqrt(),
            positive.powf(black_box(2.25)), positive.powf(black_box(1.66)),
            positive.ln(), x.clamp(-700., 700.).exp(), x.clamp(-1., 1.).asin(),
            x.floor(), x.ceil(), x.round(), x.powi(2), x.rem_euclid(y),
            x.cos() * y + x, x * (1. + y) + y,
            x.sin().atan2(x.cos()),
            (x.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * 65536.).round(),
        ] {
            println!("{:016x}", black_box(result).to_bits());
        }
    }
}
