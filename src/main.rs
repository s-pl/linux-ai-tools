fn main() {
    println!("ai-linux-tools instalado.");
    println!("Comandos AI-friendly nativos en Rust:");
    println!("  als   -> listado compacto TSV (usa --pack)");
    println!("  acat  -> lectura de archivos (usa --pack)");
    println!("  agrep -> busqueda recursiva de texto TSV (usa --pack)");
    println!("  afind -> busqueda recursiva de rutas (usa --pack)");
    println!("  adu   -> tamanos de rutas en bytes (usa --pack)");
    println!("  aps   -> procesos desde /proc TSV (usa --pack)");
    println!("\nEjemplo: acat src/lib.rs --pack --max 40");
}
