use ptymark::{Config, DisplayPipeline, PipelineFactory, PipelineOptions};

fn pipeline(options: PipelineOptions) -> DisplayPipeline {
    let config = Box::leak(Box::new(Config::default()));
    PipelineFactory::new(config).build(options)
}

fn valid_input() -> &'static [u8] {
    b"before\n```openmath\n<OMOBJ xmlns=\"http://www.openmath.org/OpenMath\"><OMA><OMS cd=\"relation1\" name=\"eq\"/><OMA><OMS cd=\"arith1\" name=\"plus\"/><OMV name=\"x\"/><OMI>1</OMI></OMA><OMI>2</OMI></OMA></OMOBJ>\n```\nafter\n"
}

#[test]
fn openmath_rendering_is_independent_of_stream_chunk_boundaries() {
    let mut whole = pipeline(PipelineOptions::default());
    let mut whole_output = Vec::new();
    whole
        .feed(valid_input(), &mut whole_output)
        .expect("whole feed");
    whole.finish(&mut whole_output).expect("whole finish");

    let mut bytewise = pipeline(PipelineOptions::default());
    let mut bytewise_output = Vec::new();
    for byte in valid_input() {
        bytewise
            .feed(&[*byte], &mut bytewise_output)
            .expect("bytewise feed");
    }
    bytewise
        .finish(&mut bytewise_output)
        .expect("bytewise finish");

    assert_eq!(bytewise_output, whole_output);
    let rendered = String::from_utf8(whole_output).expect("UTF-8 preview");
    assert!(rendered.contains("x + 1 = 2"));
    assert!(!rendered.contains("```openmath"));
}

#[test]
fn malformed_openmath_falls_back_to_exact_source_in_normal_mode() {
    let input = b"```openmath\n<OMOBJ xmlns=\"http://www.openmath.org/OpenMath\"><OMR href=\"#other\"/></OMOBJ>\n```\n";
    let mut pipeline = pipeline(PipelineOptions::default());
    let mut output = Vec::new();
    pipeline.feed(input, &mut output).expect("fallback feed");
    pipeline.finish(&mut output).expect("fallback finish");

    assert_eq!(output, input);
    assert_eq!(pipeline.report().fallback_blocks, 1);
}

#[test]
fn strict_openmath_failure_happens_before_replacement_bytes() {
    let input = b"```openmath\n<OMOBJ xmlns=\"http://www.openmath.org/OpenMath\"><OMR href=\"#other\"/></OMOBJ>\n```\n";
    let mut pipeline = pipeline(PipelineOptions {
        strict: true,
        ..PipelineOptions::default()
    });
    let mut output = Vec::new();
    let error = pipeline
        .feed(input, &mut output)
        .expect_err("strict conversion failure");

    assert!(error.to_string().contains("OpenMath"));
    assert!(output.is_empty());
}

#[test]
fn source_and_safe_modes_preserve_openmath_exactly() {
    for options in [
        PipelineOptions {
            source: true,
            ..PipelineOptions::default()
        },
        PipelineOptions {
            safe: true,
            ..PipelineOptions::default()
        },
    ] {
        let mut pipeline = pipeline(options);
        let mut output = Vec::new();
        pipeline
            .feed(valid_input(), &mut output)
            .expect("mode feed");
        pipeline.finish(&mut output).expect("mode finish");
        assert_eq!(output, valid_input());
    }
}

#[test]
fn protected_terminal_regions_never_reach_openmath_conversion() {
    let input = b"\x1b[?1049h```openmath\n<OMOBJ xmlns=\"http://www.openmath.org/OpenMath\"><OMI>1</OMI></OMOBJ>\n```\n\x1b[?1049l";
    let mut pipeline = pipeline(PipelineOptions::default());
    let mut output = Vec::new();
    for chunk in input.chunks(3) {
        pipeline.feed(chunk, &mut output).expect("protected feed");
    }
    pipeline.finish(&mut output).expect("protected finish");
    assert_eq!(output, input);
}
