//! Perfil estructural efectivo, consumible por herramientas sin dependencias JSON.
use emllm::native::{NativeLimits, Resource};

fn main() {
    let limits = NativeLimits::default();
    let resources = [
        Resource::PackageBytes,
        Resource::Tensors,
        Resource::BaseNodes,
        Resource::TemplateInstructions,
        Resource::Ports,
        Resource::Roots,
        Resource::ClosedNodes,
        Resource::ReachableNodes,
        Resource::NumericSlots,
    ];
    println!("{{");
    for (i, resource) in resources.iter().enumerate() {
        println!(
            "  \"{}\": {}{}",
            resource.as_str(),
            limits.limit(*resource),
            if i + 1 == resources.len() { "" } else { "," }
        );
    }
    println!("}}");
}
