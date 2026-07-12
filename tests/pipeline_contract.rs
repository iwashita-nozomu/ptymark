use ptymark::{
    DetectionConfig, DisplayPipeline, FencedDetector, MemoryCache, NoopCache, PreviewRenderer,
    RenderContext, RenderService, SourceRenderer,
};

fn pipeline(source: bool, cache: bool) -> DisplayPipeline {
    let detector = Box::new(FencedDetector::new(&DetectionConfig::default()));
    let renderer: Box<dyn ptymark::Renderer> = if source {
        Box::new(SourceRenderer)
    } else {
        Box::new(PreviewRenderer)
    };
    let cache: Box<dyn ptymark::ArtifactCache> = if cache {
        Box::new(MemoryCache::new(16, 64 * 1024))
    } else {
        Box::new(NoopCache::default())
    };
    DisplayPipeline::new(
        detector,
        RenderService::new(renderer, cache),
        RenderContext {
            columns: 80,
            color: false,
            theme_fingerprint: 0,
        },
        false,
    )
}

#[test]
fn chunk_boundaries_do_not_change_output() {
    let input = b"before\n```mermaid\nflowchart LR\n  A --> B\n```\nafter\n";

    let mut one_byte = pipeline(false, false);
    let mut one_byte_output = Vec::new();
    for byte in input {
        one_byte
            .feed(&[*byte], &mut one_byte_output)
            .expect("one-byte feed");
    }
    one_byte.finish(&mut one_byte_output).expect("finish");

    let mut whole = pipeline(false, false);
    let mut whole_output = Vec::new();
    whole.feed(input, &mut whole_output).expect("whole feed");
    whole.finish(&mut whole_output).expect("finish");

    assert_eq!(one_byte_output, whole_output);
    assert!(!whole_output.windows(3).any(|window| window == b"```"));
}

#[test]
fn source_mode_is_lossless() {
    let input = b"before\n$$\nE = mc^2\n$$\nafter\n";
    let mut pipeline = pipeline(true, false);
    let mut output = Vec::new();
    pipeline.feed(input, &mut output).expect("feed");
    pipeline.finish(&mut output).expect("finish");
    assert_eq!(output, input);
}

#[test]
fn invalid_utf8_block_falls_back_to_exact_source() {
    let input = b"before\n$$\n\xff\n$$\nafter\n";
    let mut pipeline = pipeline(false, true);
    let mut output = Vec::new();
    pipeline.feed(input, &mut output).expect("feed");
    pipeline.finish(&mut output).expect("finish");
    assert_eq!(output, input);
    assert_eq!(pipeline.report().fallback_blocks, 1);
}

#[test]
fn control_sequences_are_never_detected_as_markdown() {
    let input = b"\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049l";
    let mut pipeline = pipeline(false, true);
    let mut output = Vec::new();
    for chunk in input.chunks(2) {
        pipeline.feed(chunk, &mut output).expect("feed");
    }
    pipeline.finish(&mut output).expect("finish");
    assert_eq!(output, input);
}
