use emllm::model_io::{
    F32Bits, LoadError, LoadErrorKind as K, LoadLimits, TensorKind, read_package, read_stories260k,
};

const LIMITS: LoadLimits = LoadLimits {
    max_bytes: 16 * 1024 * 1024,
    max_tensors: 128,
};

fn golden() -> Vec<u8> {
    let hex: String = include_str!("fixtures/tiny-package-v1.hex")
        .split_whitespace()
        .collect();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn put32(data: &mut [u8], offset: usize, n: u32) {
    data[offset..offset + 4].copy_from_slice(&n.to_le_bytes());
}

fn put64(data: &mut [u8], offset: usize, n: u64) {
    data[offset..offset + 8].copy_from_slice(&n.to_le_bytes());
}

// Modelo estructural de pesos cero: no representa el checkpoint ni sus logits.
fn structural_model(count: usize) -> Vec<u8> {
    let mut data = vec![0; 64 + 48 * count];
    data[..8].copy_from_slice(b"EMLTINY\0");
    for (i, n) in [1, count as u32, 64, 172, 5, 8, 4, 512, 512, 16]
        .into_iter()
        .enumerate()
    {
        put32(&mut data, 8 + 4 * i, n);
    }
    put64(&mut data, 48, 64);
    put64(&mut data, 56, (64 + 48 * count) as u64);
    let mut catalog = vec![(1, u32::MAX, vec![512, 64])];
    for layer in 0..5 {
        for (kind, shape) in [
            (2, vec![64]),
            (3, vec![64, 64]),
            (4, vec![32, 64]),
            (5, vec![32, 64]),
            (6, vec![64, 64]),
            (7, vec![64]),
            (8, vec![172, 64]),
            (9, vec![64, 172]),
            (10, vec![172, 64]),
        ] {
            catalog.push((kind, layer, shape));
        }
    }
    for (kind, shape) in [
        (11, vec![64]),
        (12, vec![512, 64]),
        (13, vec![16, 4]),
        (14, vec![16, 4]),
        (15, vec![]),
        (16, vec![]),
    ] {
        catalog.push((kind, u32::MAX, shape));
    }
    for (i, (kind, layer, shape)) in catalog.into_iter().take(count).enumerate() {
        let base = 64 + 48 * i;
        let size = 4 * shape.iter().product::<usize>();
        let offset = if kind == 12 {
            64 + 48 * count
        } else {
            data.len()
        };
        put32(&mut data, base, kind);
        put32(&mut data, base + 4, layer);
        put32(&mut data, base + 8, shape.len() as u32);
        put32(&mut data, base + 12, if kind == 12 { 0 } else { u32::MAX });
        put64(&mut data, base + 16, *shape.first().unwrap_or(&1) as u64);
        put64(&mut data, base + 24, *shape.get(1).unwrap_or(&1) as u64);
        put64(&mut data, base + 32, offset as u64);
        put64(&mut data, base + 40, size as u64);
        if kind != 12 {
            data.resize(offset + size, 0);
            match kind {
                15 => put32(&mut data, offset, 0x3727c5ac),
                16 => put32(&mut data, offset, 0x3eb504f3),
                _ => (),
            }
        }
    }
    data
}

#[test]
fn shared_python_fixture_preserves_every_bit_and_alias() {
    let bytes = golden();
    let package = read_package(&bytes, LIMITS).unwrap();
    assert_eq!(package.config().dim, 2);
    let tensors = package.tensors();
    assert_eq!(tensors.len(), 2);
    assert_eq!(tensors[0].kind(), TensorKind::Embedding);
    assert_eq!(tensors[0].shape(), &[2, 2]);
    assert_eq!(tensors[1].alias(), Some(0));
    assert!(std::ptr::eq(tensors[0].bytes(), tensors[1].bytes()));
    assert_eq!(
        tensors[0].scalars().map(F32Bits::bits).collect::<Vec<_>>(),
        [0x80000000, 0x3f800000, 0xbf000000, 0x40000000]
    );
    assert_eq!(
        tensors[0]
            .scalars()
            .map(F32Bits::to_f64)
            .collect::<Vec<_>>(),
        [-0.0, 1.0, -0.5, 2.0]
    );
    assert_eq!(
        read_stories260k(&bytes, LIMITS).unwrap_err(),
        LoadError {
            kind: K::Configuration,
            tensor: None,
            field: "dim"
        }
    );
}

#[test]
fn every_truncated_fixture_is_rejected_and_limits_are_inclusive() {
    let bytes = golden();
    for end in 0..bytes.len() {
        assert!(read_package(&bytes[..end], LIMITS).is_err(), "end={end}");
    }
    read_package(
        &bytes,
        LoadLimits {
            max_bytes: 176,
            max_tensors: 2,
        },
    )
    .unwrap();
    for max_bytes in [0, 175] {
        assert_eq!(
            read_package(
                &bytes,
                LoadLimits {
                    max_bytes,
                    ..LIMITS
                }
            )
            .unwrap_err()
            .kind,
            K::ByteLimit
        );
    }
    for max_tensors in [0, 1] {
        assert_eq!(
            read_package(
                &bytes,
                LoadLimits {
                    max_tensors,
                    ..LIMITS
                }
            )
            .unwrap_err()
            .kind,
            K::TensorLimit
        );
    }
    let mut declared = bytes.clone();
    put64(&mut declared, 104, 16 * 1024 * 1024);
    assert_eq!(
        read_package(&declared, LIMITS).unwrap_err().kind,
        K::ByteLimit
    );
    let mut extra = bytes;
    extra.push(0);
    assert_eq!(
        read_package(&extra, LIMITS).unwrap_err().field,
        "trailing_bytes"
    );
}

#[test]
fn rejects_malformed_header_directory_and_alias_with_locations() {
    for (offset, value, kind, tensor, field) in [
        (0, 0, K::Magic, None, "magic"),
        (8, 2, K::Version, None, "version"),
        (16, 0, K::Dimension, None, "dim"),
        (48, 63, K::Layout, None, "directory"),
        (56, 164, K::Layout, None, "directory"),
        (64, 99, K::TensorKind, Some(0), "kind"),
        (68, 0, K::TensorKind, Some(0), "layer"),
        (72, 3, K::Shape, Some(0), "dimensions"),
        (80, 0, K::Shape, Some(0), "dimensions"),
        (96, 164, K::Layout, Some(0), "offset"),
        (104, 12, K::Shape, Some(0), "size"),
        (112, 1, K::DuplicateTensor, Some(1), "kind/layer"),
        (124, 1, K::Alias, Some(1), "target"),
        (144, 164, K::Alias, Some(1), "layout"),
    ] {
        let mut bytes = golden();
        put32(&mut bytes, offset, value);
        assert_eq!(
            read_package(&bytes, LIMITS).unwrap_err(),
            LoadError {
                kind,
                tensor,
                field
            },
            "offset={offset}"
        );
    }
    for (offset, value) in [(80, 1_u64 << 63), (96, u64::MAX)] {
        let mut bytes = golden();
        put64(&mut bytes, offset, value);
        assert_eq!(
            read_package(&bytes, LIMITS).unwrap_err().kind,
            K::SizeOverflow
        );
    }
}

#[test]
fn nonfinite_values_and_alias_chains_are_rejected() {
    for bits in [0x7f800000, 0xff800000, 0x7fc00000, 0x7f800001, 0xffc00000] {
        assert!(F32Bits::new(bits).is_none());
        let mut bytes = golden();
        put32(&mut bytes, 160, bits);
        assert_eq!(
            read_package(&bytes, LIMITS).unwrap_err(),
            LoadError {
                kind: K::NonFinite,
                tensor: Some(0),
                field: "scalar"
            }
        );
    }
    assert_eq!(
        F32Bits::new(1).unwrap().to_f64(),
        f64::from(f32::from_bits(1))
    );
    let mut bytes = structural_model(52);
    // Hacer que la entrada 48 comparta forma/offset con la salida alias 47.
    let target = bytes[64 + 48 * 47 + 8..64 + 48 * 48].to_vec();
    bytes[64 + 48 * 48 + 8..64 + 48 * 49].copy_from_slice(&target);
    put32(&mut bytes, 64 + 48 * 48 + 12, 47);
    assert_eq!(read_package(&bytes, LIMITS).unwrap_err().kind, K::Alias);
}

#[test]
fn validates_full_model_shapes_inventory_and_constants() {
    let bytes = structural_model(52);
    let model = read_stories260k(&bytes, LIMITS).unwrap();
    assert_eq!(model.package().tensors().len(), 52);
    assert_eq!(
        model.tensor(TensorKind::Down, Some(4)).unwrap().shape(),
        &[64, 172]
    );
    assert!(model.tensor(TensorKind::Query, Some(5)).is_none());
    assert_eq!(
        model
            .tensor(TensorKind::Epsilon, None)
            .unwrap()
            .scalars()
            .next()
            .unwrap()
            .bits(),
        0x3727c5ac
    );
    assert_eq!(
        model.tensor(TensorKind::Output, None).unwrap().alias(),
        Some(0)
    );
    let mut wrong = bytes.clone();
    put32(&mut wrong, 28, 7);
    assert_eq!(
        read_stories260k(&wrong, LIMITS).unwrap_err().field,
        "n_heads"
    );
    let mut wrong = bytes.clone();
    put64(&mut wrong, 64 + 48 * 3 + 16, 64);
    put64(&mut wrong, 64 + 48 * 3 + 24, 32);
    read_package(&wrong, LIMITS).unwrap();
    assert_eq!(
        read_stories260k(&wrong, LIMITS).unwrap_err(),
        LoadError {
            kind: K::Shape,
            tensor: Some(3),
            field: "model_shape"
        }
    );
    assert_eq!(
        read_stories260k(&structural_model(51), LIMITS)
            .unwrap_err()
            .kind,
        K::Inventory
    );
    for offset in [bytes.len() - 8, bytes.len() - 4] {
        let mut wrong = bytes.clone();
        put32(&mut wrong, offset, 0);
        assert_eq!(
            read_stories260k(&wrong, LIMITS).unwrap_err().kind,
            K::Constant
        );
    }
}
