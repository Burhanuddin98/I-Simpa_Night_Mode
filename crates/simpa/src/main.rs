use std::path::Path;
use std::process::ExitCode;

use simpa_core::formats;

const USAGE: &str = "usage:
  simpa --version
  simpa dump <cbin|mbin|poly|tetgen|gabe|csbin|pbin> <file>   canonical dump of a solver file
  simpa gen <cbin|mbin|poly> <seed> <out>                     write a generated model, print its dump
                                                              (poly: as upstream's reader sees it)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    match argv.as_slice() {
        [] | ["--version"] | ["version"] => {
            println!(
                "simpa {} (solvers from upstream {})",
                simpa_core::VERSION,
                &simpa_core::SOLVER_COMMIT[..7]
            );
            ExitCode::SUCCESS
        }
        ["dump", format, file] => dump(format, Path::new(file)),
        ["gen", format, seed, out] => match seed.parse::<u64>() {
            Ok(seed) => generate(format, seed, Path::new(out)),
            Err(_) => fail(&format!("seed must be an unsigned integer, got '{seed}'")),
        },
        [command, ..] => fail(&format!("unknown command '{command}'\n{USAGE}")),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("simpa: {msg}");
    ExitCode::from(2)
}

fn dump(format: &str, file: &Path) -> ExitCode {
    let text = match format {
        "cbin" => formats::cbin::dump_file(file),
        "mbin" => formats::mbin::dump_file(file),
        "poly" => formats::poly::dump_file(file),
        "tetgen" => formats::tetgen::dump_file(file),
        "gabe" => formats::gabe::dump_file(file),
        "csbin" => formats::csbin::dump_file(file),
        "pbin" => formats::pbin::dump_file(file),
        other => return fail(&format!("unknown format '{other}'")),
    };
    print!("{text}");
    if text.starts_with("error") {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn generate(format: &str, seed: u64, out: &Path) -> ExitCode {
    let written = match format {
        "cbin" => {
            let m = formats::cbin::generate(seed);
            formats::cbin::write_file(&m, out).map(|_| formats::cbin::dump(&m))
        }
        "mbin" => {
            let m = formats::mbin::generate(seed);
            formats::mbin::write_file(&m, out).map(|_| formats::mbin::dump(&m))
        }
        "poly" => {
            // Printed as upstream's ImportPOLY reads it back: its bug at poly.cpp:406-424 gives
            // every user facet after the first the first one's marker. See formats::poly docs.
            let m = formats::poly::generate(seed);
            formats::poly::write_file(&m, out)
                .map(|_| formats::poly::dump(&formats::poly::upstream_import_view(&m)))
        }
        other => return fail(&format!("no writer for format '{other}'")),
    };
    match written {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(&format!("writing {}: {e}", out.display())),
    }
}
