use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

pub const OPENMATH_NAMESPACE: &str = "http://www.openmath.org/OpenMath";
pub const OPENMATH_TO_TEX_ID: &str = "builtin/openmath-to-tex-v1";

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_XML_DEPTH: usize = 128;
const MAX_XML_NODES: usize = 8192;

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
/// The converter is deliberately local and bounded. It performs no Content
/// Dictionary lookup, entity resolution, file access, or network access.
/// Unknown symbols remain representable through a generic `cd.name` operator.
pub fn to_tex(input: &[u8]) -> Result<String, OpenMathError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(OpenMathError::new(format!(
            "input exceeds the {MAX_INPUT_BYTES} byte OpenMath limit"
        )));
    }
    let source = std::str::from_utf8(input)
        .map_err(|error| OpenMathError::new(format!("input is not valid UTF-8: {error}")))?;
    let root = XmlParser::new(source).parse_document()?;
    if local_name(&root.name) != "OMOBJ" {
        return Err(OpenMathError::new("root element must be OMOBJ"));
    }
    validate_openmath_namespace(&root)?;
    let children = object_children(&root)?;
    if children.len() != 1 {
        return Err(OpenMathError::new(
            "OMOBJ must contain exactly one OpenMath object",
        ));
    }
    let object = parse_object(children[0], 0)?;
    Ok(render_object(&object))
}

fn validate_openmath_namespace(root: &XmlElement) -> Result<(), OpenMathError> {
    let namespace_key = root
        .name
        .split_once(':')
        .map_or_else(|| "xmlns".to_owned(), |(prefix, _)| format!("xmlns:{prefix}"));
    match root.attributes.get(&namespace_key).map(String::as_str) {
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

fn parse_object(element: &XmlElement, depth: usize) -> Result<OpenMathObject, OpenMathError> {
    if depth >= MAX_XML_DEPTH {
        return Err(OpenMathError::new(format!(
            "OpenMath object nesting exceeds {MAX_XML_DEPTH} levels"
        )));
    }

    match local_name(&element.name) {
        "OMS" => {
            require_empty(element)?;
            Ok(OpenMathObject::Symbol(Symbol {
                cd: required_attribute(element, "cd")?.to_owned(),
                name: required_attribute(element, "name")?.to_owned(),
            }))
        }
        "OMV" => {
            require_empty(element)?;
            Ok(OpenMathObject::Variable(
                required_attribute(element, "name")?.to_owned(),
            ))
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
                || !value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '='))
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
            let operator = parse_object(operator, depth + 1)?;
            let arguments = arguments
                .iter()
                .map(|argument| parse_object(argument, depth + 1))
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
            let symbol = parse_object(symbol, depth + 1)?;
            if !matches!(symbol, OpenMathObject::Symbol(_)) {
                return Err(OpenMathError::new("OME must start with OMS"));
            }
            let arguments = arguments
                .iter()
                .map(|argument| parse_object(argument, depth + 1))
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

fn parse_float(element: &XmlElement) -> Result<OpenMathObject, OpenMathError> {
    require_empty(element)?;
    match (
        element.attributes.get("dec"),
        element.attributes.get("hex"),
    ) {
        (Some(decimal), None) if valid_decimal(decimal) => {
            Ok(OpenMathObject::Float(decimal.clone()))
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

fn parse_binding(element: &XmlElement, depth: usize) -> Result<OpenMathObject, OpenMathError> {
    let children = object_children(element)?;
    if children.len() != 3 || local_name(&children[1].name) != "OMBVAR" {
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

fn parse_attributed(element: &XmlElement, depth: usize) -> Result<OpenMathObject, OpenMathError> {
    let children = object_children(element)?;
    if children.len() != 2 || local_name(&children[0].name) != "OMATP" {
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

fn required_attribute<'a>(
    element: &'a XmlElement,
    name: &str,
) -> Result<&'a str, OpenMathError> {
    element
        .attributes
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            OpenMathError::new(format!(
                "{} requires a non-empty {name} attribute",
                local_name(&element.name)
            ))
        })
}

fn require_empty(element: &XmlElement) -> Result<(), OpenMathError> {
    if leaf_text(element)?.trim().is_empty() {
        Ok(())
    } else {
        Err(OpenMathError::new(format!(
            "{} must not contain text",
            local_name(&element.name)
        )))
    }
}

fn leaf_text(element: &XmlElement) -> Result<String, OpenMathError> {
    let mut text = String::new();
    for child in &element.children {
        match child {
            XmlChild::Text(value) => text.push_str(value),
            XmlChild::Element(_) => {
                return Err(OpenMathError::new(format!(
                    "{} must not contain child elements",
                    local_name(&element.name)
                )));
            }
        }
    }
    Ok(text)
}

fn object_children(element: &XmlElement) -> Result<Vec<&XmlElement>, OpenMathError> {
    let mut children = Vec::new();
    for child in &element.children {
        match child {
            XmlChild::Element(element) => children.push(element),
            XmlChild::Text(text) if text.trim().is_empty() => {}
            XmlChild::Text(_) => {
                return Err(OpenMathError::new(format!(
                    "{} contains unexpected text",
                    local_name(&element.name)
                )));
            }
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
    if let OpenMathObject::Symbol(symbol) = operator {
        if let Some(rendered) = render_known_application(symbol, arguments) {
            return rendered;
        }
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
        ("arith1", "times") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\cdot"))
        }
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
        ("arith1", "abs") if arguments.len() == 1 => Some(format!(
            "\\left|{}\\right|",
            render_object(&arguments[0])
        )),
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
        ("logic1", "and") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\land"))
        }
        ("logic1", "or") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\lor"))
        }
        ("logic1", "not") => unary("\\neg "),
        ("logic1", "implies") => binary("\\Rightarrow"),
        ("logic1", "equivalent") => binary("\\Leftrightarrow"),
        ("set1", "in") => binary("\\in"),
        ("set1", "notin") => binary("\\notin"),
        ("set1", "subset") => binary("\\subseteq"),
        ("set1", "prsubset") => binary("\\subset"),
        ("set1", "union") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\cup"))
        }
        ("set1", "intersect") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\cap"))
        }
        ("set1", "setdiff") => binary("\\setminus"),
        ("set1", "cartesian_product") if !arguments.is_empty() => {
            Some(join_infix(arguments, "\\times"))
        }
        ("set1", "set") => Some(format!(
            "\\left\\{{{}\\right\\}}",
            render_arguments(arguments)
        )),
        ("list1", "list") => Some(format!(
            "\\left[{}\\right]",
            render_arguments(arguments)
        )),
        ("interval1", "integer_interval") if arguments.len() == 2 => Some(format!(
            "\\left[{}, {}\\right]_{{\\mathbb{{Z}}}}",
            render_object(&arguments[0]),
            render_object(&arguments[1])
        )),
        ("interval1", "interval") | ("interval1", "interval_cc")
            if arguments.len() == 2 =>
        {
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

#[derive(Clone, Debug)]
struct XmlElement {
    name: String,
    attributes: BTreeMap<String, String>,
    children: Vec<XmlChild>,
}

#[derive(Clone, Debug)]
enum XmlChild {
    Element(XmlElement),
    Text(String),
}

struct XmlParser<'a> {
    source: &'a str,
    offset: usize,
    nodes: usize,
}

impl<'a> XmlParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            nodes: 0,
        }
    }

    fn parse_document(mut self) -> Result<XmlElement, OpenMathError> {
        if self.source.starts_with('\u{feff}') {
            self.offset = '\u{feff}'.len_utf8();
        }
        self.skip_misc()?;
        if self.starts_with_ascii_case_insensitive("<!DOCTYPE") {
            return Err(OpenMathError::new("DOCTYPE is not permitted"));
        }
        let root = self.parse_element(0)?;
        self.skip_misc()?;
        if self.offset != self.source.len() {
            return Err(self.error("unexpected data after the root element"));
        }
        Ok(root)
    }

    fn parse_element(&mut self, depth: usize) -> Result<XmlElement, OpenMathError> {
        if depth >= MAX_XML_DEPTH {
            return Err(self.error(format!(
                "XML nesting exceeds {MAX_XML_DEPTH} levels"
            )));
        }
        self.expect("<")?;
        if self.starts_with("/") || self.starts_with("!") || self.starts_with("?") {
            return Err(self.error("expected an element name"));
        }
        let name = self.parse_name()?;
        let mut attributes = BTreeMap::new();
        let self_closing = loop {
            self.skip_whitespace();
            if self.consume("/>") {
                break true;
            }
            if self.consume(">") {
                break false;
            }
            let attribute_name = self.parse_name()?;
            self.skip_whitespace();
            self.expect("=")?;
            self.skip_whitespace();
            let attribute_value = self.parse_attribute_value()?;
            if attributes
                .insert(attribute_name.clone(), attribute_value)
                .is_some()
            {
                return Err(self.error(format!(
                    "duplicate XML attribute `{attribute_name}`"
                )));
            }
        };

        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_XML_NODES {
            return Err(self.error(format!(
                "XML element count exceeds {MAX_XML_NODES}"
            )));
        }
        if self_closing {
            return Ok(XmlElement {
                name,
                attributes,
                children: Vec::new(),
            });
        }

        let mut children = Vec::new();
        loop {
            if self.offset >= self.source.len() {
                return Err(self.error(format!("unclosed XML element `{name}`")));
            }
            if self.consume("</") {
                let closing = self.parse_name()?;
                if closing != name {
                    return Err(self.error(format!(
                        "closing element `{closing}` does not match `{name}`"
                    )));
                }
                self.skip_whitespace();
                self.expect(">")?;
                break;
            }
            if self.starts_with("<!--") {
                self.skip_comment()?;
            } else if self.starts_with("<![CDATA[") {
                children.push(XmlChild::Text(self.parse_cdata()?));
            } else if self.starts_with("<?") {
                self.skip_processing_instruction()?;
            } else if self.starts_with_ascii_case_insensitive("<!DOCTYPE") {
                return Err(self.error("DOCTYPE is not permitted"));
            } else if self.starts_with("<!") {
                return Err(self.error("XML declarations other than comments and CDATA are not permitted"));
            } else if self.starts_with("<") {
                children.push(XmlChild::Element(self.parse_element(depth + 1)?));
            } else {
                let text = self.parse_text()?;
                if !text.is_empty() {
                    children.push(XmlChild::Text(text));
                }
            }
        }

        Ok(XmlElement {
            name,
            attributes,
            children,
        })
    }

    fn skip_misc(&mut self) -> Result<(), OpenMathError> {
        loop {
            self.skip_whitespace();
            if self.starts_with("<!--") {
                self.skip_comment()?;
            } else if self.starts_with("<?") {
                self.skip_processing_instruction()?;
            } else {
                return Ok(());
            }
        }
    }

    fn skip_comment(&mut self) -> Result<(), OpenMathError> {
        self.expect("<!--")?;
        let end = self
            .rest()
            .find("-->")
            .ok_or_else(|| self.error("unclosed XML comment"))?;
        self.offset += end + 3;
        Ok(())
    }

    fn skip_processing_instruction(&mut self) -> Result<(), OpenMathError> {
        self.expect("<?")?;
        let end = self
            .rest()
            .find("?>")
            .ok_or_else(|| self.error("unclosed processing instruction"))?;
        self.offset += end + 2;
        Ok(())
    }

    fn parse_cdata(&mut self) -> Result<String, OpenMathError> {
        self.expect("<![CDATA[")?;
        let end = self
            .rest()
            .find("]]>")
            .ok_or_else(|| self.error("unclosed CDATA section"))?;
        let value = self.rest()[..end].to_owned();
        self.offset += end + 3;
        Ok(value)
    }

    fn parse_attribute_value(&mut self) -> Result<String, OpenMathError> {
        let quote = self
            .bump()
            .ok_or_else(|| self.error("missing XML attribute value"))?;
        if quote != '"' && quote != '\'' {
            return Err(self.error("XML attribute values must be quoted"));
        }
        let mut raw = String::new();
        loop {
            let character = self
                .bump()
                .ok_or_else(|| self.error("unclosed XML attribute value"))?;
            if character == quote {
                break;
            }
            if character == '<' {
                return Err(self.error("XML attribute values must not contain `<`"));
            }
            raw.push(character);
        }
        decode_entities(&raw).map_err(|message| self.error(message))
    }

    fn parse_text(&mut self) -> Result<String, OpenMathError> {
        let start = self.offset;
        while self.offset < self.source.len() && !self.starts_with("<") {
            self.bump();
        }
        decode_entities(&self.source[start..self.offset]).map_err(|message| self.error(message))
    }

    fn parse_name(&mut self) -> Result<String, OpenMathError> {
        let start = self.offset;
        while let Some(character) = self.peek() {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | ':' | '-' | '.') {
                self.bump();
            } else {
                break;
            }
        }
        if self.offset == start {
            return Err(self.error("expected an XML name"));
        }
        Ok(self.source[start..self.offset].to_owned())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    fn starts_with(&self, pattern: &str) -> bool {
        self.rest().starts_with(pattern)
    }

    fn starts_with_ascii_case_insensitive(&self, pattern: &str) -> bool {
        self.rest()
            .get(..pattern.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(pattern))
    }

    fn consume(&mut self, pattern: &str) -> bool {
        if self.starts_with(pattern) {
            self.offset += pattern.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, pattern: &str) -> Result<(), OpenMathError> {
        if self.consume(pattern) {
            Ok(())
        } else {
            Err(self.error(format!("expected `{pattern}`")))
        }
    }

    fn rest(&self) -> &'a str {
        &self.source[self.offset..]
    }

    fn error(&self, message: impl Into<String>) -> OpenMathError {
        OpenMathError::new(format!("{} at byte {}", message.into(), self.offset))
    }
}

fn decode_entities(value: &str) -> Result<String, String> {
    let mut decoded = String::new();
    let mut remaining = value;
    while let Some(index) = remaining.find('&') {
        decoded.push_str(&remaining[..index]);
        remaining = &remaining[index + 1..];
        let end = remaining
            .find(';')
            .ok_or_else(|| "unclosed XML entity".to_owned())?;
        let entity = &remaining[..end];
        let replacement = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            numeric if numeric.starts_with("#x") || numeric.starts_with("#X") => {
                let value = u32::from_str_radix(&numeric[2..], 16)
                    .map_err(|_| format!("invalid hexadecimal entity `&{entity};`"))?;
                valid_xml_character(value)
                    .ok_or_else(|| format!("invalid XML character `&{entity};`"))?
            }
            numeric if numeric.starts_with('#') => {
                let value = numeric[1..]
                    .parse::<u32>()
                    .map_err(|_| format!("invalid decimal entity `&{entity};`"))?;
                valid_xml_character(value)
                    .ok_or_else(|| format!("invalid XML character `&{entity};`"))?
            }
            _ => return Err(format!("unsupported XML entity `&{entity};`")),
        };
        decoded.push(replacement);
        remaining = &remaining[end + 1..];
    }
    decoded.push_str(remaining);
    Ok(decoded)
}

fn valid_xml_character(value: u32) -> Option<char> {
    let character = char::from_u32(value)?;
    ((value == 0x9 || value == 0xa || value == 0xd)
        || (0x20..=0xd7ff).contains(&value)
        || (0xe000..=0xfffd).contains(&value)
        || (0x10000..=0x10ffff).contains(&value))
    .then_some(character)
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
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
        assert_eq!(
            to_tex(source).expect("convert"),
            "\\text{A \\& B π}"
        );
    }

    #[test]
    fn rejects_doctype_and_external_entity_surfaces() {
        let source = br#"<!DOCTYPE OMOBJ [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<OMOBJ xmlns="http://www.openmath.org/OpenMath"><OMSTR>&xxe;</OMSTR></OMOBJ>"#;
        assert!(to_tex(source).expect_err("doctype").to_string().contains("DOCTYPE"));
    }

    #[test]
    fn rejects_cross_object_references() {
        let source = br#"<OMOBJ xmlns="http://www.openmath.org/OpenMath"><OMR href="#other"/></OMOBJ>"#;
        assert!(to_tex(source).expect_err("reference").to_string().contains("OMR"));
    }

    #[test]
    fn requires_the_openmath_namespace() {
        let source = br#"<OMOBJ><OMI>1</OMI></OMOBJ>"#;
        assert!(to_tex(source).expect_err("namespace").to_string().contains("declare"));
    }
}
