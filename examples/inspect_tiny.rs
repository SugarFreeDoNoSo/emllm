//! Valida el paquete real y permite comparar todos sus bits con el exportador.
use emllm::model_io::{LoadLimits, read_stories260k};
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: inspect_tiny PATH [--bits]")?;
    let bits = match args.next().as_deref() {
        None => false,
        Some("--bits") => true,
        _ => return Err("usage: inspect_tiny PATH [--bits]".into()),
    };
    if args.next().is_some() {
        return Err("usage: inspect_tiny PATH [--bits]".into());
    }
    let limits = LoadLimits {
        max_bytes: 16 * 1024 * 1024,
        max_tensors: 128,
    };
    let mut data = Vec::new();
    File::open(path)?
        .take(limits.max_bytes as u64 + 1)
        .read_to_end(&mut data)?;
    let model = read_stories260k(&data, limits)?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(out, "{{\"bytes\":{},\"tensors\":[", data.len())?;
    for (i, tensor) in model.package().tensors().iter().enumerate() {
        if i != 0 {
            writeln!(out, ",")?;
        }
        write!(
            out,
            "{{\"kind\":{},\"layer\":{},\"shape\":{:?},\"alias\":{},\"scalars\":{}",
            tensor.kind() as u32,
            tensor.layer().map_or("null".to_owned(), |n| n.to_string()),
            tensor.shape(),
            tensor.alias().map_or("null".to_owned(), |n| n.to_string()),
            tensor.scalars().len()
        )?;
        if bits {
            write!(out, ",\"bits\":[")?;
            for (j, scalar) in tensor.scalars().enumerate() {
                if j != 0 {
                    write!(out, ",")?;
                }
                write!(out, "{}", scalar.bits())?;
            }
            write!(out, "]")?;
        }
        write!(out, "}}")?;
    }
    writeln!(out, "]}}")?;
    out.flush()?;
    Ok(())
}
