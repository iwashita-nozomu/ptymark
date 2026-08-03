"""Preserve the documented alpha CLI contracts while using Clap's typed model."""

from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = Path(path)
    source = target.read_text(encoding="utf-8")
    if old in source:
        target.write_text(source.replace(old, new, 1), encoding="utf-8")
        return
    if new in source:
        return
    raise RuntimeError(f"{label}: neither source nor replacement fragment was found in {path}")


def main() -> None:
    replace_once(
        "src/cli.rs",
        """    disable_help_subcommand = true,
    arg_required_else_help = true
)]""",
        """    disable_help_subcommand = true,
    arg_required_else_help = true,
    after_help = "Common commands:\n  ptymark engine check\n  ptymark shell [--source|--safe] [--private] -- COMMAND [ARG...]"
)]""",
        "top-level help examples",
    )
    replace_once(
        "src/cli.rs",
        """pub fn run_from(arguments: Vec<OsString>) -> Result<i32, String> {
    let arguments = normalize_legacy_shell(arguments);
    let argv = std::iter::once(OsString::from("ptymark")).chain(arguments);""",
        """pub fn run_from(arguments: Vec<OsString>) -> Result<i32, String> {
    let arguments = normalize_legacy_shell(arguments);
    validate_render_conflicts(&arguments)?;
    let argv = std::iter::once(OsString::from("ptymark")).chain(arguments);""",
        "pre-parse render-mode validation",
    )
    replace_once(
        "src/cli.rs",
        """fn normalize_legacy_shell(mut arguments: Vec<OsString>) -> Vec<OsString> {""",
        """fn validate_render_conflicts(arguments: &[OsString]) -> Result<(), String> {
    let mut source = false;
    let mut safe = false;
    for argument in arguments {
        match argument.to_str() {
            Some("--") => break,
            Some("--source") => source = true,
            Some("--safe") => safe = true,
            _ => {}
        }
    }
    if source && safe {
        return Err("`--source` and `--safe` cannot be combined".to_owned());
    }
    Ok(())
}

fn normalize_legacy_shell(mut arguments: Vec<OsString>) -> Vec<OsString> {""",
        "render-mode conflict validator",
    )


if __name__ == "__main__":
    main()
