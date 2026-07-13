use crate::runtime::PipelineOptions;
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) fn apply_render_option(
    option: &str,
    iterator: &mut impl Iterator<Item = OsString>,
    pipeline: &mut PipelineOptions,
    config_path: &mut Option<PathBuf>,
) -> Result<bool, String> {
    match option {
        "--source" => pipeline.source = true,
        "--strict" => pipeline.strict = true,
        "--no-cache" => pipeline.no_cache = true,
        "--color" => pipeline.color = true,
        "--columns" => pipeline.columns = Some(next_columns(iterator)?),
        "--config" => set_once(config_path, next_path(iterator, "--config")?, "--config")?,
        _ => return Ok(false),
    }
    Ok(true)
}

pub(crate) fn set_once<T>(target: &mut Option<T>, value: T, option: &str) -> Result<(), String> {
    if target.replace(value).is_some() {
        return Err(format!("`{option}` may be specified only once"));
    }
    Ok(())
}

pub(crate) fn next_value(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, String> {
    iterator
        .next()
        .ok_or_else(|| format!("missing value after `{option}`"))
}

pub(crate) fn next_path(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<PathBuf, String> {
    next_value(iterator, option).map(PathBuf::from)
}

fn next_columns(iterator: &mut impl Iterator<Item = OsString>) -> Result<u16, String> {
    let value = next_value(iterator, "--columns")?
        .into_string()
        .map_err(|_| "`--columns` requires UTF-8 digits".to_owned())?;
    let columns = value
        .parse::<u16>()
        .map_err(|_| "`--columns` requires a positive integer".to_owned())?;
    if columns == 0 {
        return Err("`--columns` must be greater than zero".to_owned());
    }
    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::{apply_render_option, set_once};
    use crate::runtime::PipelineOptions;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn render_options_share_one_parser() {
        let mut pipeline = PipelineOptions::default();
        let mut config = None;
        let mut values = vec![OsString::from("120")].into_iter();

        assert!(
            apply_render_option("--columns", &mut values, &mut pipeline, &mut config,)
                .expect("columns")
        );
        assert_eq!(pipeline.columns, Some(120));
    }

    #[test]
    fn single_value_options_reject_duplicates() {
        let mut value = None;
        set_once(&mut value, PathBuf::from("one"), "--config").expect("first");
        let error = set_once(&mut value, PathBuf::from("two"), "--config").expect_err("duplicate");
        assert_eq!(error, "`--config` may be specified only once");
    }
}
