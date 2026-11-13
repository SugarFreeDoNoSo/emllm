//! Protocolo común de fixtures; los oráculos quedan fuera de la estructura Input.
use emllm::model_io::F32Bits;
use std::error::Error;

pub struct Input {
    pub operation: String,
    pub left: Vec<F32Bits>,
    pub right: Vec<F32Bits>,
    pub epsilon: Option<F32Bits>,
}
pub struct Case {
    pub input: Input,
    pub expected: Vec<f64>,
}
fn bits(text: &str) -> Result<F32Bits, Box<dyn Error>> {
    F32Bits::new(u32::from_str_radix(text, 16)?).ok_or_else(|| "nonfinite input".into())
}
pub fn parse(text: &str) -> Result<Vec<Case>, Box<dyn Error>> {
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or("missing header")?
        .split_whitespace()
        .collect::<Vec<_>>();
    if header.len() != 2 || header[0] != "EMLCOMP1" {
        return Err("invalid header".into());
    }
    let total = header[1].parse::<usize>()?;
    if total == 0 || total > 10_000 {
        return Err("case count limit".into());
    }
    let mut cases = Vec::new();
    for _ in 0..total {
        let fields = lines
            .next()
            .ok_or("missing case")?
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err("invalid case header".into());
        }
        let length = fields[1].parse::<usize>()?;
        if length == 0 || length > 512 {
            return Err("vector length limit".into());
        }
        let epsilon = match fields[0] {
            "rmsnorm" if length == 64 => Some(bits(fields[2])?),
            "softmax" if fields[2] == "0" => None,
            "dot" | "swiglu" if fields[2] == "-" => None,
            _ => return Err("invalid operation, dimension or auxiliary field".into()),
        };
        let left = lines
            .next()
            .ok_or("missing left vector")?
            .split_whitespace()
            .map(bits)
            .collect::<Result<Vec<_>, _>>()?;
        let right_line = lines.next().ok_or("missing right vector")?;
        let right = if fields[0] == "softmax" {
            if right_line != "-" {
                return Err("unexpected right vector".into());
            }
            Vec::new()
        } else {
            right_line
                .split_whitespace()
                .map(bits)
                .collect::<Result<Vec<_>, _>>()?
        };
        let expected = lines
            .next()
            .ok_or("missing oracle")?
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()?;
        if left.len() != length
            || (fields[0] != "softmax" && right.len() != length)
            || expected.len() != if fields[0] == "dot" { 1 } else { length }
            || expected.iter().any(|x| !x.is_finite())
        {
            return Err("invalid vector or oracle shape/content".into());
        }
        cases.push(Case {
            input: Input {
                operation: fields[0].to_owned(),
                left,
                right,
                epsilon,
            },
            expected,
        });
    }
    if lines.next().is_some() {
        return Err("trailing data".into());
    }
    Ok(cases)
}
