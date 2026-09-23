use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--version") | Some("version") => {
            println!(
                "simpa {} (solvers from upstream {})",
                simpa_core::VERSION,
                &simpa_core::SOLVER_COMMIT[..7]
            );
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("simpa: unknown command '{other}'");
            ExitCode::from(2)
        }
    }
}
