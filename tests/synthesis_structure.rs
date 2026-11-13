//! Huellas congeladas antes de extraer las expansiones: orden de pares y raíces.
use emllm::{
    expr::Arena,
    program::compile,
    synth::{self, Rational, model, scalar},
};

#[test]
fn expansions_keep_their_closed_structural_fingerprint() {
    let mut arena = Arena::new(100_000);
    let x = synth::rational(&mut arena, Rational::new(-1, 2).unwrap()).unwrap();
    let y = synth::rational(&mut arena, Rational::new(2, 1).unwrap()).unwrap();
    let mut roots = vec![
        synth::zero(&mut arena).unwrap(),
        synth::exp(&mut arena, &x).unwrap(),
        synth::add(&mut arena, &x, &y).unwrap(),
        synth::sub(&mut arena, &x, &y).unwrap(),
        synth::mul(&mut arena, &x, &y).unwrap(),
        synth::neg(&mut arena, &x).unwrap(),
    ];
    for bits in [0, 0x8000_0000, 1, 0x3d14_ebf3, 0xbf80_0000, 0x4400_0000] {
        roots.push(scalar::synthesize(&mut arena, scalar::F32Bits::new(bits).unwrap()).unwrap());
    }
    roots.extend([
        model::add(&mut arena, &x, &y).unwrap(),
        model::sub(&mut arena, &x, &y).unwrap(),
        model::mul(&mut arena, &x, &y).unwrap(),
        model::neg(&mut arena, &x).unwrap(),
        model::add_positive(&mut arena, &y, &y).unwrap(),
        model::divide_positive(&mut arena, &y, &y).unwrap(),
        model::reciprocal_positive(&mut arena, &y).unwrap(),
        model::inverse_sqrt(&mut arena, &y).unwrap(),
        model::silu(&mut arena, &x).unwrap(),
    ]);
    let program = compile(&arena, &roots, 100_000).unwrap();
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for index in program.pairs().iter().flatten().chain(program.roots()) {
        for byte in (*index as u64).to_le_bytes() {
            fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    assert_eq!(
        (
            arena.node_count(),
            program.node_count(),
            program.roots().len()
        ),
        (1897, 1897, 21)
    );
    assert_eq!(fingerprint, 0x9a15_e07c_5363_9f6f);
}
