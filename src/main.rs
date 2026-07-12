use std::{env, process};

fn main() {
    if let Some(result) = ptymark::managed_launcher::run_if_managed_alias() {
        match result {
            Ok(code) => process::exit(code),
            Err(message) => {
                eprintln!("ptymark managed launcher: {message}");
                process::exit(2);
            }
        }
    }

    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if ptymark::filtered_run::is_top_level_help(&arguments) {
        print!("{}{}", ptymark::cli::HELP, ptymark::filtered_run::HELP);
        return;
    }
    if let Some(result) = ptymark::filtered_run::run_from(arguments) {
        match result {
            Ok(code) => process::exit(code),
            Err(message) => {
                eprintln!("ptymark: {message}");
                process::exit(2);
            }
        }
    }

    ptymark::cli::main_entry();
}
