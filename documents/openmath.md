<!--
@dependency-start
contract design
responsibility Defines the supported OpenMath input, conversion, fallback, and extension contract.
upstream design ../README.md user-facing entrypoint
upstream standard https://openmath.org/standard/om20-2019-07-01/omstd20.html OpenMath 2 standard
upstream implementation ../src/openmath.rs bounded XML-to-TeX converter
downstream implementation ../src/detector.rs explicit OpenMath fence detection
downstream implementation ../src/format_adapter.rs structured math adaptation
downstream implementation ../tests/openmath_contract.rs end-to-end safety and fallback evidence
@dependency-end
-->

# OpenMath input

Ptymark accepts XML-encoded OpenMath objects as an explicit, line-bounded block. OpenMath is a source format for the existing `math` semantic role; it is not a separate executable engine.

```text
safe terminal text
  -> explicit `openmath` fence
  -> bounded OpenMath XML parser
  -> deterministic OpenMath-to-TeX adapter
  -> configured math route: preview, source, or MathJax
  -> existing terminal-safe presenter and exact-source fallback
```

This keeps detection, rendering policy, cache identity, presentation, and recovery consistent with TeX block math.

## Input form

Use a fenced block whose opening and closing markers are complete lines:

````text
```openmath
<OMOBJ xmlns="http://www.openmath.org/OpenMath" version="2.0">
  <OMA>
    <OMS cd="relation1" name="eq"/>
    <OMA>
      <OMS cd="arith1" name="plus"/>
      <OMV name="x"/>
      <OMI>1</OMI>
    </OMA>
    <OMI>2</OMI>
  </OMA>
</OMOBJ>
```
````

Raw `OMOBJ` text outside this fence is ordinary terminal output. Ptymark does not use inline XML heuristics and does not scan prompts, cursor-addressed output, alternate-screen applications, progress redraws, or other protected terminal regions.

## Configuration

OpenMath shares the current math detection and engine policy:

```toml
[detection]
math = true

[engines.math]
backend = "mathjax-cli" # preview | source | mathjax-cli
path = "/absolute/path/to/tex2svg"
```

`math = false` disables TeX, `math|latex|tex` fences, dollar-sign block math, and OpenMath fences together. This avoids a configuration schema change for what remains one semantic renderer role.

Per-invocation recovery remains unchanged:

```text
ptymark --source -- COMMAND  detect blocks but emit exact source
ptymark --safe -- COMMAND    bypass semantic detection and rendering
ptymark --private -- COMMAND render without the process-local artifact cache
```

## Supported XML object model

The first implementation accepts one `OMOBJ` root in the OpenMath namespace and supports these object constructors:

| Constructor | Handling |
| --- | --- |
| `OMS`, `OMV` | Content Dictionary symbol and variable |
| `OMI`, `OMF` | Integer and decimal/hexadecimal floating value |
| `OMSTR`, `OMB` | String and base64 byte value |
| `OMA` | Function or operator application |
| `OMBIND`, `OMBVAR` | Binding and bound variables |
| `OME` | OpenMath error object, rendered explicitly as an error term |
| `OMATTR`, `OMATP` | Attributed object and attribute pairs |
| nested `OMOBJ` | One nested object |

`OMR` references are rejected in this bounded slice. Resolving references would require a separately reviewed identity, cycle, and capture policy.

The XML parser accepts an XML declaration, comments, CDATA, built-in XML entities, and numeric character references. It rejects `DOCTYPE`, custom entities, external entities, unknown declarations, malformed nesting, unsupported constructors, excessive depth, excessive node count, and input above the semantic block limit.

## Content Dictionary rendering

Ptymark does not download or execute Content Dictionaries. Common official symbols receive readable TeX presentation, including representative symbols from:

- `arith1`, `relation1`, and `logic1`;
- `set1`, `setname1`, `list1`, and `interval1`;
- `nums1`, `alg1`, and `integer1`;
- `transc1`, `calculus1`, `quant1`, and `fns1`.

An unknown symbol remains usable and visible. For example:

```xml
<OMS cd="research1" name="wave_operator"/>
```

is rendered through a deterministic generic form equivalent to:

```tex
\operatorname{research1.wave\_operator}
```

This is deliberate: project-specific and research Content Dictionaries do not require a network registry merely to pass through the terminal renderer. Adding a specialized presentation later changes only the local symbol mapping and its tests, not detection or terminal safety.

## Failure and safety behavior

The exact fenced source remains attached to the semantic block throughout conversion and rendering.

- In normal mode, malformed or unsupported OpenMath restores the complete original fenced source.
- In strict mode, conversion fails before any replacement bytes are committed.
- In source mode, XML conversion is not attempted; even malformed XML is emitted exactly.
- In safe mode, the detector and every semantic renderer are bypassed.
- Failed conversions are not cached.
- No parser path reads files, resolves external entities, contacts the network, launches a shell, or retrieves a remote CD.

The existing one-MiB semantic block limit is the outer input bound. The converter additionally limits XML nesting and element count before producing TeX.

## Deliberate non-goals

The current slice does not provide:

- inline or heuristic XML recognition;
- OpenMath JSON encoding;
- `OMR` reference graphs;
- remote Content Dictionary lookup;
- semantic validation against CD signatures or formal properties;
- round-trip OpenMath editing;
- a new OpenMath executable or installer role.

These are independent extensions. They must not weaken the explicit-block detector or byte-exact terminal bypass.

## Try it

A complete runnable sample is in [`../examples/openmath.md`](../examples/openmath.md):

```bash
ptymark preview examples/openmath.md
```

Use source mode to verify lossless recovery:

```bash
ptymark preview --source examples/openmath.md
```

## Standards and dictionaries

- [OpenMath 2 standard](https://openmath.org/standard/om20-2019-07-01/omstd20.html)
- [OpenMath technical overview](https://openmath.org/technical/)
- [Official Content Dictionaries](https://openmath.org/cd/)
