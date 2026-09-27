fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "typescript".to_string());
    match mode.as_str() {
        "typescript" => print!("{}", ostudy_lib::ipc_wire_contract::render_typescript()),
        "fixtures" => print!("{}", ostudy_lib::ipc_wire_contract::render_fixtures_json()),
        _ => {
            eprintln!("usage: export-ipc-wire-contract [typescript|fixtures]");
            std::process::exit(2);
        }
    }
}
