fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "json".to_string());
    match mode.as_str() {
        "json" => print!("{}", ostudy_lib::migration_contract::render_json()),
        _ => {
            eprintln!("usage: export-migration-contract [json]");
            std::process::exit(2);
        }
    }
}
