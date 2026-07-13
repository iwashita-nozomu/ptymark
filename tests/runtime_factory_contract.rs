use ptymark::{Config, PipelineFactory, PipelineOptions};

fn execute(options: PipelineOptions, source: &[u8]) -> (Vec<u8>, usize) {
    let config = Config::default();
    let mut pipeline = PipelineFactory::new(&config).build(options);
    let mut output = Vec::new();
    pipeline.feed(source, &mut output).expect("feed");
    pipeline.finish(&mut output).expect("finish");
    (output, pipeline.report().cache_hits)
}

#[test]
fn source_mode_is_lossless_through_the_shared_factory() {
    let source = b"$$\nE = mc^2\n$$\n";
    let (output, cache_hits) = execute(
        PipelineOptions {
            source: true,
            ..PipelineOptions::default()
        },
        source,
    );
    assert_eq!(output, source);
    assert_eq!(cache_hits, 0);
}

#[test]
fn configured_cache_is_shared_by_every_runtime_path() {
    let source = b"$$\nE = mc^2\n$$\n$$\nE = mc^2\n$$\n";
    let (output, cache_hits) = execute(PipelineOptions::default(), source);
    assert!(!output.windows(2).any(|window| window == b"$$"));
    assert_eq!(cache_hits, 1);
}

#[test]
fn no_cache_option_disables_reuse_without_changing_output() {
    let source = b"$$\nE = mc^2\n$$\n$$\nE = mc^2\n$$\n";
    let (cached_output, _) = execute(PipelineOptions::default(), source);
    let (uncached_output, cache_hits) = execute(
        PipelineOptions {
            no_cache: true,
            ..PipelineOptions::default()
        },
        source,
    );
    assert_eq!(uncached_output, cached_output);
    assert_eq!(cache_hits, 0);
}
