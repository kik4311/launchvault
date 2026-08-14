#[derive(Debug, Clone, PartialEq)]
pub enum VdfValue {
    Str(String),
    Map(Vec<(String, VdfValue)>),
}

impl VdfValue {
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        match self {
            VdfValue::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

pub fn parse(text: &str) -> Option<VdfValue> {
    let mut p = Parser {
        chars: text.chars().peekable(),
    };
    skip_ws(&mut p);
    let key = parse_token(&mut p)?;
    skip_ws(&mut p);
    if p.chars.peek() == Some(&'{') {
        let value = parse_value(&mut p)?;
        Some(VdfValue::Map(vec![(key, value)]))
    } else {
        Some(VdfValue::Str(key))
    }
}

fn skip_ws(p: &mut Parser) {
    while let Some(&c) = p.chars.peek() {
        if c.is_whitespace() {
            p.chars.next();
        } else {
            break;
        }
    }
}

fn parse_value(p: &mut Parser) -> Option<VdfValue> {
    skip_ws(p);
    let c = *p.chars.peek()?;
    if c == '{' {
        p.chars.next();
        let mut entries = Vec::new();
        loop {
            skip_ws(p);
            match p.chars.peek() {
                None => return None,
                Some('}') => {
                    p.chars.next();
                    break;
                }
                _ => {}
            }
            let key = parse_token(p)?;
            skip_ws(p);
            let value = parse_value(p)?;
            entries.push((key, value));
        }
        Some(VdfValue::Map(entries))
    } else {
        parse_token(p).map(VdfValue::Str)
    }
}

fn parse_token(p: &mut Parser) -> Option<String> {
    let c = *p.chars.peek()?;
    if c == '"' {
        p.chars.next();
        let mut s = String::new();
        loop {
            match p.chars.next()? {
                '"' => break,
                '\\' => {
                    if let Some(&e) = p.chars.peek() {
                        if e == '"' {
                            s.push('"');
                            p.chars.next();
                        }
                    }
                }
                ch => s.push(ch),
            }
        }
        Some(s)
    } else {
        None
    }
}