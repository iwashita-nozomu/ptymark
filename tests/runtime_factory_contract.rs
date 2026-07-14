use ptymark::{CacheStats, Config, MathEngine, PipelineFactory, PipelineOptions, PipelineReport};
use std::path::PathBuf;

fn execute_with_config(
    config: &Config,
    options: PipelineOptions,
    source: &[u8],
) -> (Vec<u8>, PipelineReport, CacheStats) {
    let mut pipeline = PipelineFactory::new(config).build(options);
    let mut output = Vec::new();
    pipeline.feed(source, &mut output).expect("feed");
    pipeline.finish(&mut output).expect("finish");
    let report = pipeline.report().clone();
    let cache = pipeline.cache_stats();
    (output, report, cache)
}

fn execute(options: PipelineOptions, source: &[u8]) -> (Vec<u8>, PipelineReport, CacheStats) {
    execute_with_config(&Config::default(), options, source)
}

#[test]
fn source_mode_is_lossless_through_the_shared_factory() {
    let source = b"$$\nE = mc^2\n$$\n";
    let (output, report, cache) = execute(
        PipelineOptions {
            source: true,
            ..PipelineOptions::default()
        },
        source,
    );
    assert_eq!(output, source);
    assert_eq!(report.semantic_blocks, 1);
    assert_eq!(report.cache_hits, 0);
    assert_eq!(cache.entries, 0);
}

#[test]
fn source_and_safe_modes_never_start_external_engines() {
    let mut config = Config::default();
    config.engines.math.backend = MathEngine::MathjaxCli;
    config.engines.math.path = PathBuf::from("ptymark-engine-that-must-not-run");
    config.rendering.strict = true;
    let source = b"$$\nE = mc^2\n$$\n";

    for (options, expected_blocks) in [
        (
            PipelineOptions {
                source: true,
                ..PipelineOptions::default()
            },
            1,
        ),
        (
            PipelineOptions {
                safe: true,
                ..PipelineOptions::default()
            },
            0,
        ),
    ] {
        let (output, report, cache) = execute_with_config(&config, options, source);
        assert_eq!(output, source);
        assert_eq!(report.semantic_blocks, expected_blocks);
        assert_eq!(report.rendered_blocks, expected_blocks);
        assert_eq!(report.fallback_blocks, 0);
        assert_eq!(cache.entries, 0);
    }
}

#[test]
fn configured_cache_is_shared_by_every_runtime_path() {
    let source = b"$$\nE = mc^2\n$$\n$$\nE = mc^2\n$$\n";
    let (output, report, _) = execute(PipelineOptions::default(), source);
    assert!(!output.windows(2).any(|window| window == b"$$"));
    assert_eq!(report.cache_hits, 1);
}

#[test]
fn no_cache_option_disables_reuse_without_changing_output() {
    let source = b"$$\nE = mc^2\n$$\n$$\nE = mc^2\n$$\n";
    let (cached_output, _, _) = execute(PipelineOptions::default(), source);
    let (uncached_output, report, cache) = execute(
        PipelineOptions {
            no_cache: true,
            ..PipelineOptions::default()
        },
        source,
    );
    assert_eq!(uncached_output, cached_output);
    assert_eq!(report.cache_hits, 0);
    assert_eq!(cache.entries, 0);
}

#[test]
fn private_mode_uses_noop_cache_without_changing_rendering() {
    let source = b"$$\nE = mc^2\n$$\n$$\nE = mc^2\n$$\n";
    let (cached_output, _, _) = execute(PipelineOptions::default(), source);
    let (private_output, report, cache) = execute(
        PipelineOptions {
            private: true,
            ..PipelineOptions::default()
        },
        source,
    );
    assert_eq!(private_output, cached_output);
    assert_eq!(report.semantic_blocks, 2);
    assert_eq!(report.cache_hits, 0);
    assert_eq!(cache.entries, 0);
}
