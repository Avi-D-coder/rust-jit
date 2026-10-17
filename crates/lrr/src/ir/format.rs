//! `builtin # format_args(template, args..)` templates (rustc_parse_format
//! grammar), parsed once at lowering and rendered by the format operation.
//! Rendering follows core::fmt for the closed scalar/str family; any other
//! argument is refused until trait-method formatting joins the call path.
use crate::core::{diagnostic::Diagnostic, digest::StableEncoder};

fn syntax(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-FORMAT-TEMPLATE", message.into())
}
fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FormatTrait {
    Display,
    Debug,
    LowerHexDebug,
    UpperHexDebug,
    LowerHex,
    UpperHex,
    Octal,
    Binary,
    LowerExp,
    UpperExp,
    Pointer,
}
/// Resolved count: a literal or the index of a `usize` argument.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Count {
    Is(usize),
    Argument(usize),
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Placeholder {
    pub argument: usize,
    pub fill: char,
    pub align: Option<Align>,
    pub plus: bool,
    pub minus: bool,
    pub alternate: bool,
    pub zero: bool,
    pub width: Option<Count>,
    pub precision: Option<Count>,
    pub format: FormatTrait,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Piece {
    Literal(String),
    Placeholder(Placeholder),
}
/// Parsed and resolved template. `arguments` is the number of evaluated
/// argument expressions (positional, then named) it indexes.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FormatTemplate {
    pub pieces: Vec<Piece>,
    pub arguments: usize,
}

/// Unresolved argument reference from the template text.
enum ArgRef {
    Next,
    Index(usize),
    Name(String),
}
enum RawCount {
    Is(usize),
    Index(usize),
    Name(String),
    Star,
}

impl FormatTemplate {
    /// `named` lists the names of the named arguments, which follow the
    /// `positional` unnamed ones in evaluation order. An implicit capture
    /// (`{x}` with no such argument) is reported as unsupported.
    pub fn parse(template: &str, positional: usize, named: &[String]) -> Result<Self, Diagnostic> {
        let lookup_name = |name: &str| -> Result<usize, Diagnostic> {
            named
                .iter()
                .position(|n| n == name)
                .map(|i| positional + i)
                .ok_or_else(|| {
                    unsupported("format_args implicit argument capture awaits local name integration")
                })
        };
        let total = positional + named.len();
        let mut next = 0usize;
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut chars = template.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    literal.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    literal.push('}');
                }
                '}' => return Err(syntax("unmatched `}` in format string")),
                '{' => {
                    let mut inner = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => inner.push(c),
                            None => return Err(syntax("unterminated format placeholder")),
                        }
                    }
                    if !literal.is_empty() {
                        pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                    }
                    let (argument, spec) = match inner.find(':') {
                        Some(at) => (&inner[..at], &inner[at + 1..]),
                        None => (inner.as_str(), ""),
                    };
                    let argument = argument.trim();
                    let argument = if argument.is_empty() {
                        ArgRef::Next
                    } else if let Ok(index) = argument.parse::<usize>() {
                        ArgRef::Index(index)
                    } else {
                        ArgRef::Name(argument.trim_start_matches("r#").to_owned())
                    };
                    let mut placeholder = parse_spec(spec)?;
                    // `.*` takes its precision from the next positional
                    // argument, before the value's own implicit position.
                    if let Some(RawCount::Star) = &placeholder.1 {
                        placeholder.1 = Some(RawCount::Index(next));
                        next += 1;
                    }
                    let argument = match argument {
                        ArgRef::Next => {
                            let index = next;
                            next += 1;
                            index
                        }
                        ArgRef::Index(index) => index,
                        ArgRef::Name(name) => lookup_name(&name)?,
                    };
                    let resolve = |count: Option<RawCount>| -> Result<Option<Count>, Diagnostic> {
                        Ok(match count {
                            None => None,
                            Some(RawCount::Is(n)) => Some(Count::Is(n)),
                            Some(RawCount::Index(i)) => Some(Count::Argument(i)),
                            Some(RawCount::Name(name)) => Some(Count::Argument(lookup_name(&name)?)),
                            Some(RawCount::Star) => unreachable!("resolved above"),
                        })
                    };
                    let (mut spec, precision, width) = placeholder;
                    spec.precision = resolve(precision)?;
                    spec.width = resolve(width)?;
                    spec.argument = argument;
                    for index in [Some(spec.argument), count_index(spec.width), count_index(spec.precision)]
                        .into_iter()
                        .flatten()
                    {
                        if index >= total {
                            return Err(syntax("format placeholder names an absent argument"));
                        }
                    }
                    pieces.push(Piece::Placeholder(spec));
                }
                c => literal.push(c),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Literal(literal));
        }
        Ok(Self {
            pieces,
            arguments: total,
        })
    }
    pub fn encode(&self, e: &mut StableEncoder) {
        e.u64(self.arguments as u64);
        e.sequence(&self.pieces, |e, piece| match piece {
            Piece::Literal(text) => {
                e.u8(0);
                e.string(text);
            }
            Piece::Placeholder(p) => {
                e.u8(1);
                e.u64(p.argument as u64);
                e.u32(u32::from(p.fill));
                e.u8(match p.align {
                    None => 0,
                    Some(Align::Left) => 1,
                    Some(Align::Center) => 2,
                    Some(Align::Right) => 3,
                });
                e.bool(p.plus);
                e.bool(p.minus);
                e.bool(p.alternate);
                e.bool(p.zero);
                for count in [p.width, p.precision] {
                    match count {
                        None => e.u8(0),
                        Some(Count::Is(n)) => {
                            e.u8(1);
                            e.u64(n as u64);
                        }
                        Some(Count::Argument(n)) => {
                            e.u8(2);
                            e.u64(n as u64);
                        }
                    }
                }
                e.u8(p.format as u8);
            }
        });
    }
}

fn count_index(count: Option<Count>) -> Option<usize> {
    match count {
        Some(Count::Argument(index)) => Some(index),
        _ => None,
    }
}

/// `[[fill]align][sign]['#']['0'][width]['.' precision][type]`
fn parse_spec(spec: &str) -> Result<(Placeholder, Option<RawCount>, Option<RawCount>), Diagnostic> {
    let mut placeholder = Placeholder {
        argument: 0,
        fill: ' ',
        align: None,
        plus: false,
        minus: false,
        alternate: false,
        zero: false,
        width: None,
        precision: None,
        format: FormatTrait::Display,
    };
    let chars: Vec<char> = spec.chars().collect();
    let mut at = 0;
    let align = |c: char| match c {
        '<' => Some(Align::Left),
        '^' => Some(Align::Center),
        '>' => Some(Align::Right),
        _ => None,
    };
    if chars.len() >= 2 && let Some(a) = align(chars[1]) {
        placeholder.fill = chars[0];
        placeholder.align = Some(a);
        at = 2;
    } else if let Some(a) = chars.first().copied().and_then(align) {
        placeholder.align = Some(a);
        at = 1;
    }
    match chars.get(at) {
        Some('+') => {
            placeholder.plus = true;
            at += 1;
        }
        Some('-') => {
            placeholder.minus = true;
            at += 1;
        }
        _ => {}
    }
    if chars.get(at) == Some(&'#') {
        placeholder.alternate = true;
        at += 1;
    }
    // A `0` flag precedes the width unless it is itself `0$`.
    if chars.get(at) == Some(&'0') && chars.get(at + 1) != Some(&'$') {
        placeholder.zero = true;
        at += 1;
    }
    let width = parse_count(&chars, &mut at, false)?;
    let mut precision = None;
    if chars.get(at) == Some(&'.') {
        at += 1;
        precision = Some(
            parse_count(&chars, &mut at, true)?
                .ok_or_else(|| syntax("format precision is absent"))?,
        );
    }
    let rest: String = chars[at..].iter().collect();
    placeholder.format = match rest.trim() {
        "" => FormatTrait::Display,
        "?" => FormatTrait::Debug,
        "x?" => FormatTrait::LowerHexDebug,
        "X?" => FormatTrait::UpperHexDebug,
        "x" => FormatTrait::LowerHex,
        "X" => FormatTrait::UpperHex,
        "o" => FormatTrait::Octal,
        "b" => FormatTrait::Binary,
        "e" => FormatTrait::LowerExp,
        "E" => FormatTrait::UpperExp,
        "p" => FormatTrait::Pointer,
        _ => return Err(syntax("unknown format trait")),
    };
    Ok((placeholder, precision, width))
}

fn parse_count(chars: &[char], at: &mut usize, precision: bool) -> Result<Option<RawCount>, Diagnostic> {
    if precision && chars.get(*at) == Some(&'*') {
        *at += 1;
        return Ok(Some(RawCount::Star));
    }
    let start = *at;
    let mut end = start;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    if end == start {
        return Ok(None);
    }
    let word: String = chars[start..end].iter().collect();
    if chars.get(end) == Some(&'$') {
        *at = end + 1;
        return Ok(Some(match word.parse::<usize>() {
            Ok(index) => RawCount::Index(index),
            Err(_) => RawCount::Name(word),
        }));
    }
    match word.parse::<usize>() {
        Ok(n) => {
            *at = end;
            Ok(Some(RawCount::Is(n)))
        }
        // A trailing type such as `x` is not a count.
        Err(_) => {
            let digits = chars[start..end].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 {
                return Ok(None);
            }
            let n: String = chars[start..start + digits].iter().collect();
            *at = start + digits;
            Ok(Some(RawCount::Is(n.parse().map_err(|_| syntax("format count overflow"))?)))
        }
    }
}

/// A formattable argument value read at the format operation.
#[derive(Debug, Clone)]
pub enum FormatValue {
    Unit,
    Bool(bool),
    Char(char),
    Str(String),
    Signed(i128, u32),
    Unsigned(u128, u32),
    F32(f32),
    F64(f64),
    /// The entries of a `[T]`/`[T; N]` (`<[T] as Debug>` is
    /// `f.debug_list().entries(self.iter()).finish()`).
    List(Vec<FormatValue>),
}

impl FormatValue {
    fn as_count(&self) -> Result<usize, Diagnostic> {
        match self {
            Self::Unsigned(n, _) => usize::try_from(*n).map_err(|_| syntax("format count overflow")),
            _ => Err(syntax("format width/precision argument is not usize")),
        }
    }
}

pub fn render(template: &FormatTemplate, values: &[FormatValue]) -> Result<String, Diagnostic> {
    let mut out = String::new();
    for piece in &template.pieces {
        match piece {
            Piece::Literal(text) => out.push_str(text),
            Piece::Placeholder(p) => {
                let count = |c: Option<Count>| -> Result<Option<usize>, Diagnostic> {
                    Ok(match c {
                        None => None,
                        Some(Count::Is(n)) => Some(n),
                        Some(Count::Argument(i)) => Some(
                            values
                                .get(i)
                                .ok_or_else(|| syntax("format count argument absent"))?
                                .as_count()?,
                        ),
                    })
                };
                let width = count(p.width)?;
                let precision = count(p.precision)?;
                let value = values
                    .get(p.argument)
                    .ok_or_else(|| syntax("format argument absent"))?;
                render_one(&mut out, p, width, precision, value)?;
            }
        }
    }
    Ok(out)
}

/// One placeholder's rendering of a native value, under resolved counts.
pub fn render_placeholder(
    p: &Placeholder,
    width: Option<usize>,
    precision: Option<usize>,
    value: &FormatValue,
) -> Result<String, Diagnostic> {
    let mut out = String::new();
    render_one(&mut out, p, width, precision, value)?;
    Ok(out)
}

fn pad(out: &mut String, p: &Placeholder, width: Option<usize>, default: Align, body: &str) {
    let len = body.chars().count();
    let Some(width) = width.filter(|w| *w > len) else {
        out.push_str(body);
        return;
    };
    let fill = width - len;
    let (before, after) = match p.align.unwrap_or(default) {
        Align::Left => (0, fill),
        Align::Right => (fill, 0),
        Align::Center => (fill / 2, fill - fill / 2),
    };
    out.extend(std::iter::repeat_n(p.fill, before));
    out.push_str(body);
    out.extend(std::iter::repeat_n(p.fill, after));
}

/// core::fmt::Formatter::pad_integral: `sign`/`prefix` then digits; the `0`
/// flag zero-pads after them and ignores fill/alignment.
fn pad_integral(out: &mut String, p: &Placeholder, width: Option<usize>, sign: &str, prefix: &str, digits: &str) {
    let len = sign.chars().count() + prefix.chars().count() + digits.chars().count();
    if p.zero && let Some(width) = width.filter(|w| *w > len) {
        out.push_str(sign);
        out.push_str(prefix);
        out.extend(std::iter::repeat_n('0', width - len));
        out.push_str(digits);
        return;
    }
    let body = format!("{sign}{prefix}{digits}");
    pad(out, p, width, Align::Right, &body);
}

fn render_one(
    out: &mut String,
    p: &Placeholder,
    width: Option<usize>,
    precision: Option<usize>,
    value: &FormatValue,
) -> Result<(), Diagnostic> {
    use FormatTrait::*;
    let debug = matches!(p.format, Debug | LowerHexDebug | UpperHexDebug);
    match value {
        // core::fmt::DebugList: every entry is formatted with the list's own
        // Formatter options; `{:#?}` puts each entry on its own line behind
        // a 4-space PadAdapter indent, followed by `,`.
        FormatValue::List(entries) if debug => {
            if !p.alternate {
                out.push('[');
                for (n, entry) in entries.iter().enumerate() {
                    if n > 0 {
                        out.push_str(", ");
                    }
                    render_one(out, p, width, precision, entry)?;
                }
                out.push(']');
            } else if entries.is_empty() {
                out.push_str("[]");
            } else {
                out.push_str("[\n");
                for entry in entries {
                    let mut text = String::new();
                    render_one(&mut text, p, width, precision, entry)?;
                    for line in text.split_inclusive('\n') {
                        out.push_str("    ");
                        out.push_str(line);
                    }
                    out.push_str(",\n");
                }
                out.push(']');
            }
        }
        FormatValue::Str(text) if p.format == Display => {
            let text = match precision {
                Some(n) => text.chars().take(n).collect::<String>(),
                None => text.clone(),
            };
            pad(out, p, width, Align::Left, &text);
        }
        FormatValue::Str(text) if debug => out.push_str(&format!("{text:?}")),
        FormatValue::Char(c) if p.format == Display => {
            pad(out, p, width, Align::Left, &c.to_string());
        }
        FormatValue::Char(c) if debug => out.push_str(&format!("{c:?}")),
        FormatValue::Bool(b) if p.format == Display || debug => {
            pad(out, p, width, Align::Left, if *b { "true" } else { "false" });
        }
        FormatValue::Unit if debug => pad(out, p, width, Align::Left, "()"),
        FormatValue::Signed(..) | FormatValue::Unsigned(..) => {
            let (negative, magnitude, bits) = match value {
                FormatValue::Signed(v, bits) => (*v < 0, v.unsigned_abs(), *bits),
                FormatValue::Unsigned(v, bits) => (false, *v, *bits),
                _ => unreachable!(),
            };
            // Radix formats print the two's complement bits of the type.
            let raw = match value {
                FormatValue::Signed(v, bits) if *bits < 128 => (*v as u128) & ((1u128 << bits) - 1),
                FormatValue::Signed(v, _) => *v as u128,
                FormatValue::Unsigned(v, _) => *v,
                _ => unreachable!(),
            };
            let _ = bits;
            let format = match p.format {
                LowerHexDebug => LowerHex,
                UpperHexDebug => UpperHex,
                Debug => Display,
                other => other,
            };
            let plus = if p.plus { "+" } else { "" };
            match format {
                Display => {
                    let sign = if negative { "-" } else { plus };
                    pad_integral(out, p, width, sign, "", &magnitude.to_string());
                }
                LowerHex => pad_integral(out, p, width, plus, if p.alternate { "0x" } else { "" }, &format!("{raw:x}")),
                UpperHex => pad_integral(out, p, width, plus, if p.alternate { "0x" } else { "" }, &format!("{raw:X}")),
                Octal => pad_integral(out, p, width, plus, if p.alternate { "0o" } else { "" }, &format!("{raw:o}")),
                Binary => pad_integral(out, p, width, plus, if p.alternate { "0b" } else { "" }, &format!("{raw:b}")),
                LowerExp | UpperExp => {
                    let body = match (value, precision) {
                        (FormatValue::Signed(v, _), Some(n)) => format!("{:.*e}", n, v.unsigned_abs()),
                        (FormatValue::Signed(v, _), None) => format!("{:e}", v.unsigned_abs()),
                        (_, Some(n)) => format!("{:.*e}", n, magnitude),
                        (_, None) => format!("{magnitude:e}"),
                    };
                    let body = if format == UpperExp { body.to_uppercase() } else { body };
                    let sign = if negative { "-" } else { plus };
                    pad_integral(out, p, width, sign, "", &body);
                }
                _ => return Err(unsupported("format trait is not implemented for integers")),
            }
        }
        FormatValue::F32(_) | FormatValue::F64(_) => {
            let body = match (value, p.format, precision) {
                (FormatValue::F32(v), Display, None) => format!("{}", v.abs()),
                (FormatValue::F32(v), Display, Some(n)) => format!("{:.*}", n, v.abs()),
                (FormatValue::F32(v), Debug, None) => format!("{:?}", v.abs()),
                (FormatValue::F32(v), Debug, Some(n)) => format!("{:.*?}", n, v.abs()),
                (FormatValue::F32(v), LowerExp, None) => format!("{:e}", v.abs()),
                (FormatValue::F32(v), LowerExp, Some(n)) => format!("{:.*e}", n, v.abs()),
                (FormatValue::F32(v), UpperExp, None) => format!("{:E}", v.abs()),
                (FormatValue::F32(v), UpperExp, Some(n)) => format!("{:.*E}", n, v.abs()),
                (FormatValue::F64(v), Display, None) => format!("{}", v.abs()),
                (FormatValue::F64(v), Display, Some(n)) => format!("{:.*}", n, v.abs()),
                (FormatValue::F64(v), Debug, None) => format!("{:?}", v.abs()),
                (FormatValue::F64(v), Debug, Some(n)) => format!("{:.*?}", n, v.abs()),
                (FormatValue::F64(v), LowerExp, None) => format!("{:e}", v.abs()),
                (FormatValue::F64(v), LowerExp, Some(n)) => format!("{:.*e}", n, v.abs()),
                (FormatValue::F64(v), UpperExp, None) => format!("{:E}", v.abs()),
                (FormatValue::F64(v), UpperExp, Some(n)) => format!("{:.*E}", n, v.abs()),
                _ => return Err(unsupported("format trait is not implemented for floats")),
            };
            let negative = match value {
                FormatValue::F32(v) => v.is_sign_negative() && !v.is_nan(),
                FormatValue::F64(v) => v.is_sign_negative() && !v.is_nan(),
                _ => unreachable!(),
            };
            let sign = if negative { "-" } else if p.plus { "+" } else { "" };
            pad_integral(out, p, width, sign, "", &body);
        }
        _ => {
            return Err(unsupported(
                "format of this argument awaits trait-method formatting",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render_str(template: &str, values: &[FormatValue]) -> String {
        render(&FormatTemplate::parse(template, values.len(), &[]).unwrap(), values).unwrap()
    }
    #[test]
    fn templates_render_like_core_fmt() {
        use FormatValue::*;
        assert_eq!(render_str("a{{b}}c", &[]), "a{b}c");
        assert_eq!(render_str("{} {}", &[Signed(-5, 32), Str("x".into())]), "-5 x");
        assert_eq!(render_str("{1} {0}", &[Bool(true), Char('c')]), "c true");
        assert_eq!(render_str("{:5}|{:<5}|{:^5}|{:>5}", &[Signed(42, 32), Str("ab".into()), Str("ab".into()), Str("ab".into())]), "   42|ab   | ab  |   ab");
        assert_eq!(render_str("{:05}", &[Signed(-42, 32)]), "-0042");
        assert_eq!(render_str("{:+}", &[Signed(42, 32)]), "+42");
        assert_eq!(render_str("{:#x} {:X} {:o} {:#b}", &[Unsigned(255, 8), Unsigned(255, 8), Unsigned(8, 8), Unsigned(5, 8)]), "0xff FF 10 0b101");
        assert_eq!(render_str("{:x}", &[Signed(-1, 8)]), "ff");
        assert_eq!(render_str("{:#010x}", &[Unsigned(255, 32)]), "0x000000ff");
        assert_eq!(render_str("{:?} {:?}", &[Str("a\"b".into()), Char('\n')]), "\"a\\\"b\" '\\n'");
        assert_eq!(render_str("{:.2} {:?} {}", &[F64(1.0 / 3.0), F64(1.0), F64(1e20)]), "0.33 1.0 100000000000000000000");
        assert_eq!(render_str("{:8.3}|{:<8.3}|{:08.3}", &[F64(-1.5), F64(1.5), F64(-1.5)]), "  -1.500|1.500   |-001.500");
        assert_eq!(render_str("{:.*}", &[Unsigned(1, 64), F64(2.25)]), "2.2");
        assert_eq!(render_str("{:1$}|", &[Str("a".into()), Unsigned(3, 64)]), "a  |");
        assert_eq!(render_str("{:.3}", &[Str("abcdef".into())]), "abc");
        // DebugList: entries inherit the list's options; `#` is one per line.
        let list = List(vec![Signed(1, 32), Signed(-2, 32)]);
        assert_eq!(render_str("{:?}", &[list.clone()]), "[1, -2]");
        assert_eq!(render_str("{:3?}", &[list.clone()]), "[  1,  -2]");
        assert_eq!(render_str("{:#?}", &[list.clone()]), "[\n    1,\n    -2,\n]");
        assert_eq!(render_str("{:?} {:#?}", &[List(vec![]), List(vec![])]), "[] []");
        let nested = List(vec![List(vec![Str("a".into())]), List(vec![])]);
        assert_eq!(render_str("{:?}", &[nested.clone()]), "[[\"a\"], []]");
        assert_eq!(
            render_str("{:#?}", &[nested]),
            "[\n    [\n        \"a\",\n    ],\n    [],\n]"
        );
        assert_eq!(render_str("{:e}", &[Unsigned(1234, 32)]), "1.234e3");
        assert_eq!(render_str("{:*^7}", &[Str("x".into())]), "***x***");
        let named = FormatTemplate::parse("{a}-{0}-{a}", 1, &["a".into()]).unwrap();
        assert_eq!(render(&named, &[Signed(1, 32), Signed(2, 32)]).unwrap(), "2-1-2");
        assert!(FormatTemplate::parse("{x}", 0, &[]).is_err());
        assert!(FormatTemplate::parse("}", 0, &[]).is_err());
    }
}
