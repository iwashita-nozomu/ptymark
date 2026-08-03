use crate::limits::{MAX_OPENMATH_DEPTH, MAX_OPENMATH_INPUT_BYTES, MAX_OPENMATH_NODES};
use roxmltree::{Document, Node, ParsingOptions};
use std::error::Error;
use std::fmt;

pub const OPENMATH_NAMESPACE: &str = "http://www.openmath.org/OpenMath";
pub const OPENMATH_TO_TEX_ID: &str = "builtin/openmath-to-tex-v2-roxmltree";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenMathError {
    message: String,
}

impl OpenMathError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for OpenMathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for OpenMathError {}

/// Convert one XML-encoded OpenMath object into deterministic TeX.
///
/// XML tokenization, namespace handling, duplicate-attribute checks, entity
/// decoding, and well-formedness are delegated to `roxmltree`. Ptymark owns
/// only the bounded OpenMath object model and Content Dictionary mapping.
/// DTDs and external entity resolution are disabled.
pub fn to_tex(input: &[u8]) -> Result<String, OpenMathError> {
    if input.len() > MAX_OPENMATH_INPUT_BYTES {
        return Err(OpenMathError::new(format!(
            "input exceeds the {MAX_OPENMATH_INPUT_BYTES} byte OpenMath limit"
        )));
    }
    let source = std::str::from_utf8(input)
        .map_err(|error| OpenMathError::new(format!("input is not valid UTF-8: {error}")))?;
    if contains_doctype(source) {
        return Err(OpenMathError::new("DOCTYPE is not permitted"));
    }

    let document = Document::parse_with_options(
        source,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_OPENMATH_NODES,
            entity_resolver: None,
        },
    )
    .map_err(|error| OpenMathError::new(format!("invalid OpenMath XML: {error}")))?;
    let root = document.root_element();
    if root.tag_name().name() != "OMOBJ" {
        return Err(OpenMathError::new("root element must be OMOBJ"));
    }
    validate_openmath_namespace(root)?;
    let children = object_children(root)?;
    if children.len() != 1 {
        return Err(OpenMathError::new(
            "OMOBJ must contain exactly one OpenMath object",
        ));
    }
    let object = parse_object(children[0], 0)?;
    Ok(render_object(&object))
}

fn contains_doctype(source: &str) -> bool {
    const NEEDLE: &[u8] = b"<!doctype";
    source.as_bytes().windows(NEEDLE.len()).any(|window| {
        window
            .iter()
            .zip(NEEDLE)
            .all(|(left, right)| left.to_ascii_lowercase() == *right)
    })
}

fn validate_openmath_namespace(root: Node<'_, '_>) -> Result<(), OpenMathError> {
    match root.tag_name().namespace() {
        Some(OPENMATH_NAMESPACE) => Ok(()),
        Some(namespace) => Err(OpenMathError::new(format!(
            "OMOBJ namespace `{namespace}` is not the OpenMath namespace"
        ))),
        None => Err(OpenMathError::new(format!(
            "OMOBJ must declare `{OPENMATH_NAMESPACE}`"
        ))),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Symbol {
    cd: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq)]
enum OpenMathObject {
    Symbol(Symbol),
    Variable(String),
    Integer(String),
    Float(String),
    String(String),
    Bytes(String),
    Application {
        operator: Box<OpenMathObject>,
        arguments: Vec<OpenMathObject>,
    },
    Binding {
        binder: Box<OpenMathObject>,
        variables: Vec<OpenMathObject>,
        body: Box<OpenMathObject>,
    },
    Error {
        symbol: Box<OpenMathObject>,
        arguments: Vec<OpenMathObject>,
    },
    Attributed {
        attributes: Vec<(OpenMathObject, OpenMathObject)>,
        object: Box<OpenMathObject>,
    },
}

fn parse_object(element: Node<'_, '_>, depth: usize) -> Result<OpenMathObject, OpenMathError> {
    if depth >= MAX_OPENMATH_DEPTH {
        return Err(OpenMathError::new(format!(
            "OpenMath object nesting exceeds {MAX_OPENMATH_DEPTH} levels"
        )));
    }

    match element.tag_name().name() {
        "OMS" => {
            require_empty(element)?;
            Ok(OpenMathObject::Symbol(Symbol {
                cd: required_attribute(element, "cd")?,
                name: required_attribute(element, "name")?,
            }))
        }
        "OMV" => {
            require_empty(element)?;
            Ok(OpenMathObject::Variable(required_attribute(
                element, "name",
            )?))
        }
        "OMI" => {
            let value = leaf_text(element)?.trim().to_owned();
            if !valid_integer(&value) {
                return Err(OpenMathError::new(format!(
                    "OMI contains an invalid integer `{value}`"
                )));
            }
            Ok(OpenMathObject::Integer(value))
        }
        "OMF" => parse_float(element),
        "OMSTR" => Ok(OpenMathObject::String(leaf_text(element)?)),
        "OMB" => {
            let value = leaf_text(element)?
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect::<String>();
            if value.is_empty()
                || !value.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '=')
                })
            {
                return Err(OpenMathError::new("OMB contains invalid base64 text"));
            }
            Ok(OpenMathObject::Bytes(value))
        }
        "OMA" => {
            let children = object_children(element)?;
            let (operator, arguments) = children
                .split_first()
                .ok_or_else(|| OpenMathError::new("OMA requires an operator"))?;
            let operator = parse_object(*operator, depth + 1)?;
            let arguments = arguments
                .iter()
                .map(|argument| parse_object(*argument, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(OpenMathObject::Application {
                operator: Box::new(operator),
                arguments,
            })
        }
        "OMBIND" => parse_binding(element, depth),
        "OME" => {
            let children = object_children(element)?;
            let (symbol, arguments) = children
                .split_first()
                .ok_or_else(|| OpenMathError::new("OME requires an error symbol"))?;
            let symbol = parse_object(*symbol, depth + 1)?;
            if !matches!(symbol, OpenMathObject::Symbol(_)) {
                return Err(OpenMathError::new("OME must start with OMS"));
            }
            let arguments = arguments
                .iter()
                .map(|argument| parse_object(*argument, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(OpenMathObject::Error {
                symbol: Box::new(symbol),
                arguments,
            })
        }
        "OMATTR" => parse_attributed(element, depth),
        "OMR" => Err(OpenMathError::new(
            "OMR references are not supported by the bounded converter",
        )),
        "OMOBJ" => {
            let children = object_children(element)?;
            if children.len() != 1 {
                return Err(OpenMathError::new(
                    "nested OMOBJ must contain exactly one object",
                ));
            }
            parse_object(children[0], depth + 1)
        }
        name => Err(OpenMathError::new(format!(
            "unsupported OpenMath constructor `{name}`"
        ))),
    }
}

fn parse_float(element: Node<'_, '_>) -> Result<OpenMathObject, OpenMathError> {
    require_empty(element)?;
    match (element.attribute("dec"), element.attribute("hex")) {
        (Some(decimal), None) if valid_decimal(decimal) => {
            Ok(OpenMathObject::Float(decimal.to_owned()))
        }
        (None, Some(hexadecimal))
            if !hexadecimal.is_empty()
                && hexadecimal
                    .chars()
                    .all(|character| character.is_ascii_hexdigit()) =>
        {
            Ok(OpenMathObject::Float(format!("0x{hexadecimal}")))
        }
        (Some(_), None) => Err(OpenMathError::new("OMF has an invalid dec attribute")),
        (None, Some(_)) => Err(OpenMathError::new("OMF has an invalid hex attribute")),
        _ => Err(OpenMathError::new(
            "OMF requires exactly one dec or hex attribute",
        )),
    }
}

fn parse_binding(element: Node<'_, '_>, depth: usize) -> Result<OpenMathObject, OpenMathError> {
    let children = object_children(element)?;
    if children.len() != 3 || children[1].tag_name().name() != "OMBVAR" {
        return Err(OpenMathError::new(
            "OMBIND requires binder, OMBVAR, and body children",
        ));
    }
    let binder = parse_object(children[0], depth + 1)?;
    let variables = object_children(children[1])?
        .into_iter()
        .map(|variable| parse_object(variable, depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    if variables.is_empty() {
        return Err(OpenMathError::new("OMBVAR must not be empty"));
    }
    if variables.iter().any(|variable| {
        !matches!(
            variable,
            OpenMathObject::Variable(_) | OpenMathObject::Attributed { .. }
        )
    }) {
        return Err(OpenMathError::new(
            "OMBVAR children must be OMV or attributed OMV objects",
        ));
    }
    let body = parse_object(children[2], depth + 1)?;
    Ok(OpenMathObject::Binding {
        binder: Box::new(binder),
        variables,
        body: Box::new(body),
    })
}

fn parse_attributed(element: Node<'_, '_>, depth: usize) -> Result<OpenMathObject, OpenMathError> {
    let children = object_children(element)?;
    if children.len() != 2 || children[0].tag_name().name() != "OMATP" {
        return Err(OpenMathError::new(
            "OMATTR requires OMATP followed by one object",
        ));
    }
    let attribute_children = object_children(children[0])?;
    if attribute_children.is_empty() || attribute_children.len() % 2 != 0 {
        return Err(OpenMathError::new(
            "OMATP must contain one or more key/value pairs",
        ));
    }
    let mut attributes = Vec::new();
    for pair in attribute_children.chunks_exact(2) {
        let key = parse_object(pair[0], depth + 1)?;
        if !matches!(key, OpenMathObject::Symbol(_)) {
            return Err(OpenMathError::new("OMATP keys must be OMS objects"));
        }
        let value = parse_object(pair[1], depth + 1)?;
        attributes.push((key, value));
    }
    let object = parse_object(children[1], depth + 1)?;
    Ok(OpenMathObject::Attributed {
        attributes,
        object: Box::new(object),
    })
}

fn valid_integer(value: &str) -> bool {
    let digits = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('+'))
        .unwrap_or(value);
    !digits.is_empty() && digits.chars().all(|character| character.is_ascii_digit())
}

fn valid_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.chars().any(|character| character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_digit() || matches!(character, '+' | '-' | '.' | 'e' | 'E')
        })
}

fn required_attribute(element: Node<'_, '_>, name: &str) -> Result<String, OpenMathError> {
    element
        .attribute(name)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            OpenMathError::new(format!(
                "{} requires a non-empty {name} attribute",
                element.tag_name().name()
            ))
        })
}

fn require_empty(element: Node<'_, '_>) -> Result<(), OpenMathError> {
    if leaf_text(element)?.trim().is_empty() {
        Ok(())
    } else {
        Err(OpenMathError::new(format!(
            "{} must not contain text",
            element.tag_name().name()
        )))
    }
}

fn leaf_text(element: Node<'_, '_>) -> Result<String, OpenMathError> {
    let mut text = String::new();
    for child in element.children() {
        if child.is_element() {
            return Err(OpenMathError::new(format!(
                "{} must not contain child elements",
                element.tag_name().name()
            )));
        }
        if child.is_text() {
            text.push_str(child.text().unwrap_or_default());
        }
    }
    Ok(text)
}

fn object_children<'a, 'input>(
    element: Node<'a, 'input>,
) -> Result<Vec<Node<'a, 'input>>, OpenMathError> {
    let mut children = Vec::new();
    for child in element.children() {
        if child.is_element() {
            children.push(child);
        } else if child.is_text() && !child.text().unwrap_or_default().trim().is_empty() {
            return Err(OpenMathError::new(format!(
                "{} contains unexpected text",
                element.tag_name().name()
            )));
        }
    }
    Ok(children)
}

fn render_object(object: &OpenMathObject) -> String {
    match object {
        OpenMathObject::Symbol(symbol) => render_symbol(symbol),
        OpenMathObject::Variable(name) => render_variable(name),
        OpenMathObject::Integer(value) | OpenMathObject::Float(value) => {
            if let Some(hexadecimal) = value.strip_prefix("0x") {
                format!("\\mathtt{{0x{}}}", tex_escape_text(hexadecimal))
            } else {
                value.clone()
            }
        }
        OpenMathObject::String(value) => format!("\\text{{{}}}", tex_escape_text(value)),
        OpenMathObject::Bytes(value) => format!("\\mathtt{{{}}}", tex_escape_text(value)),
        OpenMathObject::Application {
            operator,
            arguments,
        } => render_application(operator, arguments),
        OpenMathObject::Binding {
            binder,
            variables,
            body,
        } => render_binding(binder, variables, body),
        OpenMathObject::Error { symbol, arguments } => format!(
            "\\operatorname{{OpenMathError}}_{{{}}}\\left({}\\right)",
            render_object(symbol),
            render_arguments(arguments)
        ),
        OpenMathObject::Attributed { attributes, object } => {
            let rendered_attributes = attributes
                .iter()
                .map(|(key, value)| format!("{}={}", render_object(key), render_object(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "\\operatorname{{attr}}\\left({}, {}\\right)",
                render_object(object),
                rendered_attributes
            )
        }
    }
}

fn render_application(operator: &OpenMathObject, arguments: &[OpenMathObject]) -> String {
    if let OpenMathObject::Symbol(symbol) = operator
        && let Some(rendered) = render_known_application(symbol, arguments)
    {
        return rendered;
    }
    format!(
        "{}\\left({}\\right)",
        render_object(operator),
        render_arguments(arguments)
    )
}

fn render_known_application(symbol: &Symbol, arguments: &[OpenMathObject]) -> Option<String> {
    let binary = |operator: &str| {
        (arguments.len() == 2).then(|| {
            format!(
                "{} {operator} {}",
                render_object(&arguments[0]),
                render_object(&arguments[1])
            )
        })
    };
    let unary = |prefix: &str| {
        (arguments.len() == 1).then(|| format!("{prefix}{}", render_object(&arguments[0])))
    };
    let function = |name: &str| {
        Some(format!(
            "\\operatorname{{{name}}}\\left({}\\right)",
            render_arguments(arguments)
        ))
    };

    match (symbol.cd.as_str(), symbol.name.as_str()) {
        ("arith1", "plus") if !arguments.is_empty() => Some(join_infix(arguments, "+")),
        ("arith1", "times") if !arguments.is_empty() => Some(join_infix(arguments, "\\cdot")),
        ("arith1", "minus") => binary("-"),
        ("arith1", "unary_minus") => unary("-"),
        ("arith1", "divide") if arguments.len() == 2 => Some(format!(
            "\\frac{{{}}}{{{}}}",
            render_object(&arguments[0]),
            render_object(&arguments[1])
        )),
        ("arith1", "power") if arguments.len() == 2 => Some(format!(
            "{{{}}}^{{{}}}",
            render_object(&arguments[0]),
            render_object(&arguments[1])
        )),
        ("arith1", "root") if arguments.len() == 2 => Some(format!(
            "\\sqrt[{}]{{{}}}",
            render_object(&arguments[1]),
            render_object(&arguments[0])
        )),
        ("arith1", "abs") if arguments.len() == 1 => {
            Some(format!("\\left|{}\\right|", render_object(&arguments[0])))
        }
        ("arith1", "gcd") => function("gcd"),
        ("arith1", "lcm") => function("lcm"),
        ("arith1", "sum") => function("sum"),
        ("arith1", "product") => function("product"),
        ("relation1", "eq") => binary("="),
        ("relation1", "neq") => binary("\\neq"),
        ("relation1", "lt") => binary("<"),
        ("relation1", "leq") => binary("\\leq"),
        ("relation1", "gt") => binary(">"),
        ("relation1", "geq") => binary("\\geq"),
        ("relation1", "approx") => binary("\\approx"),
        ("logic1", "and") if !arguments.is_empty() => Some(join_infix(arguments, "\\land")),
        ("logic1", "or") if !arguments.is_empty() => Some(join_infix(arguments, "\\lor")),
        ("logic1", "not") => unary("\\neg "),
        ("logic1", "implies") => binary("\\Rightarrow"),
        ("logic1", "equivalent") => binary("\\Leftrightarrow"),
        ("set1", "in") => binary("\\in"),
        ("set1", "notin") => binary("\\notin"),
        ("set1", "subset") => binary("\\subseteq"),
        ("set1", "prsubset") => binary("\\subset"),
        ("set1", "union") if !arguments.is_empty() => Some(join_infix(arguments, "\\cup")),
        ("set1", "intersect") if !arguments.is_empty() => Some(join_infix(arguments, "\\cap")),
        ("set1", "setdiff") => binary("\\setminus"),
        ("set1", "cartesian_product") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\times"))
        }
        ("set1", "set") => Some(format!(
            "\\left\\{{{}\\right\\}}",
            render_arguments(arguments)
        )),
        ("list1", "list") => Some(format!("\\left[{}\\right]", render_arguments(arguments))),
        ("interval1", "integer_interval") if arguments.len() == 2 => Some(format!(
            "\\left[{}, {}\\right]_{{\\mathbb{{Z}}}}",
            render_object(&arguments[0]),
            render_object(&arguments[1])
        )),
        ("interval1", "interval") | ("interval1", "interval_cc") if arguments.len() == 2 => {
            Some(render_interval(arguments, '[', ']'))
        }
        ("interval1", "interval_co") if arguments.len() == 2 => {
            Some(render_interval(arguments, '[', ')'))
        }
        ("interval1", "interval_oc") if arguments.len() == 2 => {
            Some(render_interval(arguments, '(', ']'))
        }
        ("interval1", "interval_oo") if arguments.len() == 2 => {
            Some(render_interval(arguments, '(', ')'))
        }
        ("nums1", "rational") if arguments.len() == 2 => Some(format!(
            "\\frac{{{}}}{{{}}}",
            render_object(&arguments[0]),
            render_object(&arguments[1])
        )),
        ("nums1", "complex_cartesian") if arguments.len() == 2 => Some(format!(
            "{} + {}i",
            render_object(&arguments[0]),
            render_object(&arguments[1])
        )),
        ("integer1", "factorial") if arguments.len() == 1 => {
            Some(format!("{{{}}}!", render_object(&arguments[0])))
        }
        ("transc1", "sin") => function("sin"),
        ("transc1", "cos") => function("cos"),
        ("transc1", "tan") => function("tan"),
        ("transc1", "exp") => function("exp"),
        ("transc1", "ln") => function("ln"),
        ("transc1", "log") => function("log"),
        ("calculus1", "diff") => function("diff"),
        ("calculus1", "int") => function("int"),
        ("calculus1", "defint") => function("defint"),
        _ => None,
    }
}

fn render_binding(
    binder: &OpenMathObject,
    variables: &[OpenMathObject],
    body: &OpenMathObject,
) -> String {
    let variables = variables
        .iter()
        .map(render_object)
        .collect::<Vec<_>>()
        .join(", ");
    if let OpenMathObject::Symbol(symbol) = binder {
        match (symbol.cd.as_str(), symbol.name.as_str()) {
            ("quant1", "forall") => {
                return format!("\\forall {variables}.\\; {}", render_object(body));
            }
            ("quant1", "exists") => {
                return format!("\\exists {variables}.\\; {}", render_object(body));
            }
            ("quant1", "exists_unique") => {
                return format!("\\exists! {variables}.\\; {}", render_object(body));
            }
            ("fns1", "lambda") => {
                return format!("\\lambda {variables}.\\; {}", render_object(body));
            }
            _ => {}
        }
    }
    format!(
        "\\operatorname{{bind}}_{{{}}}\\left[{variables}\\mapsto {}\\right]",
        render_object(binder),
        render_object(body)
    )
}

fn render_symbol(symbol: &Symbol) -> String {
    match (symbol.cd.as_str(), symbol.name.as_str()) {
        ("alg1", "zero") => "0".to_owned(),
        ("alg1", "one") => "1".to_owned(),
        ("nums1", "pi") => "\\pi".to_owned(),
        ("nums1", "e") => "e".to_owned(),
        ("nums1", "i") => "i".to_owned(),
        ("nums1", "infinity") => "\\infty".to_owned(),
        ("logic1", "true") => "\\mathrm{true}".to_owned(),
        ("logic1", "false") => "\\mathrm{false}".to_owned(),
        ("setname1", "N") => "\\mathbb{N}".to_owned(),
        ("setname1", "Z") => "\\mathbb{Z}".to_owned(),
        ("setname1", "Q") => "\\mathbb{Q}".to_owned(),
        ("setname1", "R") => "\\mathbb{R}".to_owned(),
        ("setname1", "C") => "\\mathbb{C}".to_owned(),
        _ => format!(
            "\\operatorname{{{}.{}}}",
            tex_escape_operator(&symbol.cd),
            tex_escape_operator(&symbol.name)
        ),
    }
}

fn render_variable(name: &str) -> String {
    if name.chars().count() == 1 && name.chars().all(|character| character.is_alphabetic()) {
        name.to_owned()
    } else {
        format!("\\mathit{{{}}}", tex_escape_text(name))
    }
}

fn render_arguments(arguments: &[OpenMathObject]) -> String {
    arguments
        .iter()
        .map(render_object)
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_infix(arguments: &[OpenMathObject], operator: &str) -> String {
    let separator = format!(" {operator} ");
    arguments
        .iter()
        .map(render_object)
        .collect::<Vec<_>>()
        .join(&separator)
}

fn render_interval(arguments: &[OpenMathObject], left: char, right: char) -> String {
    format!(
        "\\left{left}{}, {}\\right{right}",
        render_object(&arguments[0]),
        render_object(&arguments[1])
    )
}

fn tex_escape_operator(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-') {
                character.to_string()
            } else if character == '_' {
                "\\_".to_owned()
            } else {
                format!("u{:x}", u32::from(character))
            }
        })
        .collect()
}

fn tex_escape_text(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\textbackslash{}"),
            '{' => escaped.push_str("\\{"),
            '}' => escaped.push_str("\\}"),
            '$' => escaped.push_str("\\$"),
            '&' => escaped.push_str("\\&"),
            '%' => escaped.push_str("\\%"),
            '#' => escaped.push_str("\\#"),
            '_' => escaped.push_str("\\_"),
            '^' => escaped.push_str("\\^{}"),
            '~' => escaped.push_str("\\~{}"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::to_tex;

    #[test]
    fn converts_arithmetic_and_relations() {
        let source = br#"<?xml version="1.0"?>
<OMOBJ xmlns="http://www.openmath.org/OpenMath" version="2.0">
  <OMA>
    <OMS cd="relation1" name="eq"/>
    <OMA>
      <OMS cd="arith1" name="plus"/>
      <OMV name="x"/>
      <OMI>2</OMI>
    </OMA>
    <OMI>5</OMI>
  </OMA>
</OMOBJ>"#;
        assert_eq!(to_tex(source).expect("convert"), "x + 2 = 5");
    }

    #[test]
    fn converts_quantified_bindings() {
        let source = br#"<OMOBJ xmlns="http://www.openmath.org/OpenMath">
  <OMBIND>
    <OMS cd="quant1" name="forall"/>
    <OMBVAR><OMV name="x"/></OMBVAR>
    <OMA>
      <OMS cd="set1" name="in"/>
      <OMV name="x"/>
      <OMS cd="setname1" name="R"/>
    </OMA>
  </OMBIND>
</OMOBJ>"#;
        assert_eq!(
            to_tex(source).expect("convert"),
            "\\forall x.\\; x \\in \\mathbb{R}"
        );
    }

    #[test]
    fn accepts_an_equivalent_namespace_prefix() {
        let source = br#"<om:OMOBJ xmlns:om="http://www.openmath.org/OpenMath">
  <om:OMI>1</om:OMI>
</om:OMOBJ>"#;
        assert_eq!(to_tex(source).expect("convert"), "1");
    }

    #[test]
    fn preserves_unknown_content_dictionary_symbols_generically() {
        let source = br#"<OMOBJ xmlns="http://www.openmath.org/OpenMath">
  <OMA>
    <OMS cd="research1" name="wave_operator"/>
    <OMV name="psi"/>
  </OMA>
</OMOBJ>"#;
        let rendered = to_tex(source).expect("convert");
        assert!(rendered.contains("\\operatorname{research1.wave\\_operator}"));
        assert!(rendered.contains("\\mathit{psi}"));
    }

    #[test]
    fn decodes_builtin_and_numeric_entities() {
        let source = br#"<OMOBJ xmlns="http://www.openmath.org/OpenMath">
  <OMSTR>A &amp; B &#x3c0;</OMSTR>
</OMOBJ>"#;
        assert_eq!(to_tex(source).expect("convert"), "\\text{A \\& B π}");
    }

    #[test]
    fn rejects_doctype_and_external_entity_surfaces() {
        let source = br#"<!DOCTYPE OMOBJ [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<OMOBJ xmlns="http://www.openmath.org/OpenMath"><OMSTR>&xxe;</OMSTR></OMOBJ>"#;
        assert!(
            to_tex(source)
                .expect_err("doctype")
                .to_string()
                .contains("DOCTYPE")
        );
    }

    #[test]
    fn rejects_cross_object_references() {
        let source =
            br##"<OMOBJ xmlns="http://www.openmath.org/OpenMath"><OMR href="#other"/></OMOBJ>"##;
        assert!(
            to_tex(source)
                .expect_err("reference")
                .to_string()
                .contains("OMR")
        );
    }

    #[test]
    fn requires_the_openmath_namespace() {
        let source = br#"<OMOBJ><OMI>1</OMI></OMOBJ>"#;
        assert!(
            to_tex(source)
                .expect_err("namespace")
                .to_string()
                .contains("declare")
        );
    }

    #[test]
    fn rejects_documents_above_the_node_limit() {
        let mut source = String::from(
            "<OMOBJ xmlns=\"http://www.openmath.org/OpenMath\"><OMA><OMS cd=\"list1\" name=\"list\"/>",
        );
        for index in 0..9000 {
            source.push_str(&format!("<OMI>{index}</OMI>"));
        }
        source.push_str("</OMA></OMOBJ>");
        assert!(to_tex(source.as_bytes()).is_err());
    }
}
