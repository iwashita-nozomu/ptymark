use std::{env, process};

fn exit_result(result: Result<i32, String>) -> ! {
    match result {
        Ok(code) => process::exit(code),
        Err(message) => {
            eprintln!("ptymark: {message}");
            process::exit(2);
        }
    }
}

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
        print!(
            "{}{}{}",
            ptymark::cli::HELP,
            ptymark::filtered_run::HELP,
            ptymark::interactive::HELP
        );
        return;
    }
    if let Some(result) = ptymark::filtered_run::run_from(arguments.clone()) {
        exit_result(result);
    }
    if let Some(result) = ptymark::interactive::run_from(arguments) {
        exit_result(result);
    }

    ptymark::cli::main_entry();
}
