//! Formulas as people type them: `2 * UP`, `sin(t.value)`, `axes.c2p(x, x ^ 2)`.
//!
//! This is the editor's way in and out of an [`Expr`]: [`parse`] reads what was typed,
//! [`write`] puts an expression back into the same notation, and reading what was written
//! gives the expression back. The notation is that of a spreadsheet rather than of Python:
//!
//! ```text
//! 1.5   -3   true   "hello"   #58c4dd          numbers, yes/no, text, colours
//! (1, 2)   (1, 2, 0)                             points and directions
//! UP   PI   BLUE                                 the catalogue's constants
//! circle   dot 2   `odd name!`   row[0]          objects by their names, and their parts
//! circle.center   t.value   axes.c2p(1, 2)       what an object can be asked
//! p.x   p.y                                      the parts of a point
//! + - * / ^   < <= > >= = !=   and or not        arithmetic and comparison
//! sin(x)   max(a, b)   sqrt(2)                   the catalogue's functions
//! if x > 0 then 1 else -1
//! ```
//!
//! Whether a formula makes sense — that `sin` is given a number, say — is for the checker.

use suit_core::{Arg, Args, Deck, Expr, Place, Rgba, VarId};
use suit_sig::{PyForm, Query, Signature};

/// What a formula can mention at the place it stands.
#[derive(Clone, Copy)]
pub struct Names<'a> {
    pub deck: &'a Deck,
    pub sig: &'a Signature,
    /// The objects that exist there, oldest first.
    pub objects: &'a [VarId],
    /// The values it may read: named values, then the parameters of whatever it is part
    /// of, innermost last.
    pub values: &'a [VarId],
}

/// Why a text is not a formula.
#[derive(Clone, Debug, PartialEq)]
pub struct Mistake {
    /// How many characters into the text the trouble starts.
    pub at: usize,
    pub message: String,
}

const KEYWORDS: [&str; 8] = ["if", "then", "else", "and", "or", "not", "true", "false"];

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(String),
    Word(String),
    /// A name in backticks.
    Quoted(String),
    Text(String),
    Colour(String),
    Symbol(&'static str),
}

const SYMBOLS: [&str; 21] = [
    "<=", ">=", "==", "!=", "(", ")", "[", "]", ",", ".", "+", "-", "*", "/", "^", "<", ">", "=",
    "⟨", "⟩", "×",
];

fn lex(text: &str) -> Result<Vec<(Token, usize)>, Mistake> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    let mistake = |at: usize, message: &str| Mistake {
        at,
        message: message.to_owned(),
    };
    while at < chars.len() {
        let start = at;
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
            continue;
        }
        let token = if c.is_ascii_digit() {
            while at < chars.len() && chars[at].is_ascii_digit() {
                at += 1;
            }
            // A point belongs to the number only if digits follow: `dot 2.center`.
            let digit = |index: usize| chars.get(index).is_some_and(char::is_ascii_digit);
            if chars.get(at) == Some(&'.') && digit(at + 1) {
                at += 1;
                while digit(at) {
                    at += 1;
                }
            }
            let signed = matches!(chars.get(at + 1), Some('+' | '-')) && digit(at + 2);
            if matches!(chars.get(at), Some('e' | 'E')) && (digit(at + 1) || signed) {
                at += if signed { 2 } else { 1 };
                while digit(at) {
                    at += 1;
                }
            }
            Token::Number(chars[start..at].iter().collect())
        } else if c.is_alphabetic() || c == '_' {
            while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            Token::Word(chars[start..at].iter().collect())
        } else if c == '`' {
            let end = (at + 1..chars.len()).find(|&index| chars[index] == '`');
            let end = end.ok_or_else(|| mistake(start, "This name has no closing `."))?;
            at = end + 1;
            Token::Quoted(chars[start + 1..end].iter().collect())
        } else if c == '"' {
            let mut content = String::new();
            at += 1;
            loop {
                match chars.get(at) {
                    None => return Err(mistake(start, "This text has no closing \".")),
                    Some('"') => break,
                    Some('\\') if at + 1 < chars.len() => {
                        content.push(chars[at + 1]);
                        at += 2;
                    }
                    Some(&other) => {
                        content.push(other);
                        at += 1;
                    }
                }
            }
            at += 1;
            Token::Text(content)
        } else if c == '#' {
            at += 1;
            while at < chars.len() && chars[at].is_ascii_hexdigit() {
                at += 1;
            }
            Token::Colour(chars[start..at].iter().collect())
        } else {
            let rest: String = chars[at..(at + 2).min(chars.len())].iter().collect();
            let symbol = match c {
                '≤' => Some("<="),
                '≥' => Some(">="),
                '≠' => Some("!="),
                '·' => Some("*"),
                '−' => Some("-"),
                _ => SYMBOLS.into_iter().find(|symbol| rest.starts_with(symbol)),
            };
            let Some(symbol) = symbol else {
                return Err(mistake(
                    start,
                    &format!("“{c}” means nothing in a formula."),
                ));
            };
            at += if "≤≥≠·−".contains(c) {
                1
            } else {
                symbol.chars().count()
            };
            Token::Symbol(symbol)
        };
        tokens.push((token, start));
    }
    Ok(tokens)
}

/// Reads a formula.
pub fn parse(text: &str, names: &Names) -> Result<Expr, Mistake> {
    let tokens = lex(text)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        end: text.chars().count(),
        names,
    };
    if parser.tokens.is_empty() {
        return Err(parser.mistake("Write a number, a name or a formula."));
    }
    let expr = parser.expr()?;
    match parser.peek() {
        None => Ok(expr),
        Some(_) => Err(parser.mistake("The formula could end here, but goes on.")),
    }
}

struct Parser<'a> {
    tokens: Vec<(Token, usize)>,
    at: usize,
    /// Where the text ends, for mistakes found there.
    end: usize,
    names: &'a Names<'a>,
}

type Parsed = Result<Expr, Mistake>;

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(token, _)| token)
    }

    fn mistake(&self, message: &str) -> Mistake {
        Mistake {
            at: self.tokens.get(self.at).map_or(self.end, |(_, at)| *at),
            message: message.to_owned(),
        }
    }

    fn is(&self, symbol: &str) -> bool {
        matches!(self.peek(), Some(Token::Symbol(found)) if *found == symbol)
    }

    fn eat(&mut self, symbol: &str) -> bool {
        let found = self.is(symbol);
        if found {
            self.at += 1;
        }
        found
    }

    fn word(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token::Word(found)) if found == word)
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let found = self.word(word);
        if found {
            self.at += 1;
        }
        found
    }

    fn expect(&mut self, symbol: &str) -> Result<(), Mistake> {
        match self.eat(symbol) {
            true => Ok(()),
            false => Err(self.mistake(&format!("A “{symbol}” is missing here."))),
        }
    }

    fn expr(&mut self) -> Parsed {
        if !self.eat_word("if") {
            return self.either();
        }
        let cond = self.either()?;
        if !self.eat_word("then") {
            return Err(self.mistake("After the condition comes “then”."));
        }
        let then = self.either()?;
        if !self.eat_word("else") {
            return Err(self.mistake("An “if” needs an “else”: what it is otherwise."));
        }
        let otherwise = self.expr()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then: Box::new(then),
            otherwise: Box::new(otherwise),
        })
    }

    fn either(&mut self) -> Parsed {
        let mut left = self.both()?;
        while self.eat_word("or") {
            left = Expr::call("or", vec![left, self.both()?]);
        }
        Ok(left)
    }

    fn both(&mut self) -> Parsed {
        let mut left = self.negation()?;
        while self.eat_word("and") {
            left = Expr::call("and", vec![left, self.negation()?]);
        }
        Ok(left)
    }

    fn negation(&mut self) -> Parsed {
        match self.eat_word("not") {
            true => Ok(Expr::call("not", vec![self.negation()?])),
            false => self.comparison(),
        }
    }

    fn comparison(&mut self) -> Parsed {
        let left = self.sum()?;
        let found = ["<=", ">=", "==", "!=", "<", ">", "="]
            .into_iter()
            .find(|symbol| self.is(symbol));
        let Some(symbol) = found else {
            return Ok(left);
        };
        self.at += 1;
        let fun = if symbol == "=" { "==" } else { symbol };
        Ok(Expr::call(fun, vec![left, self.sum()?]))
    }

    fn sum(&mut self) -> Parsed {
        let mut left = self.product()?;
        loop {
            let fun = match (self.is("+"), self.is("-")) {
                (true, _) => "+",
                (_, true) => "-",
                _ => return Ok(left),
            };
            self.at += 1;
            left = Expr::call(fun, vec![left, self.product()?]);
        }
    }

    fn product(&mut self) -> Parsed {
        let mut left = self.signed()?;
        loop {
            let fun = match (self.is("*") || self.is("×"), self.is("/")) {
                (true, _) => "*",
                (_, true) => "/",
                _ => return Ok(left),
            };
            self.at += 1;
            left = Expr::call(fun, vec![left, self.signed()?]);
        }
    }

    fn signed(&mut self) -> Parsed {
        if !self.is("-") {
            return self.power();
        }
        // `-2` is the number minus two, unless something binds tighter than the minus:
        // `-2 ^ 2` is minus four.
        let number = match self.tokens.get(self.at + 1) {
            Some((Token::Number(number), _)) => Some(number.clone()),
            _ => None,
        };
        let binds = |token: Option<&(Token, usize)>| {
            matches!(token, Some((Token::Symbol("^" | "." | "["), _)))
        };
        if let Some(number) = number.filter(|_| !binds(self.tokens.get(self.at + 2))) {
            self.at += 2;
            return self.number(&format!("-{number}"));
        }
        self.at += 1;
        Ok(Expr::call("neg", vec![self.signed()?]))
    }

    fn power(&mut self) -> Parsed {
        let base = self.postfix()?;
        match self.eat("^") {
            true => Ok(Expr::call("pow", vec![base, self.signed()?])),
            false => Ok(base),
        }
    }

    fn postfix(&mut self) -> Parsed {
        let mut expr = self.primary()?;
        loop {
            if self.eat("[") {
                let Expr::Place(place) = &expr else {
                    self.at -= 1;
                    return Err(self.mistake("Only an object has parts to pick out."));
                };
                let index = match self.peek() {
                    Some(Token::Number(number)) => number.parse::<u32>().ok(),
                    _ => None,
                };
                let index =
                    index.ok_or_else(|| self.mistake("A part is picked by its number: [0]."))?;
                self.at += 1;
                self.expect("]")?;
                expr = Expr::Place(place.child(index));
            } else if self.eat(".") {
                let Some(Token::Word(name)) = self.peek().cloned() else {
                    return Err(self.mistake("After the point comes what is asked for."));
                };
                expr = match expr {
                    Expr::Place(target) => self.query(target, &name)?,
                    point if ["x", "y", "z"].contains(&name.as_str()) => {
                        self.at += 1;
                        Expr::call(&name, vec![point])
                    }
                    _ => return Err(self.mistake("Of a point, only x, y and z can be taken.")),
                };
            } else {
                return Ok(expr);
            }
        }
    }

    /// `target.name` or `target.name(arguments)`, with the name's token still to be read.
    fn query(&mut self, target: Place, name: &str) -> Parsed {
        let queries = &self.names.sig.queries;
        let long = format!("get_{name}");
        let Some((query, overloads)) = queries
            .get_key_value(name)
            .or_else(|| queries.get_key_value(long.as_str()))
        else {
            let known: Vec<&str> = queries.keys().map(|query| short(query)).collect();
            let message = format!(
                "An object cannot be asked for “{name}”. It can be asked for: {}.",
                known.join(", ")
            );
            return Err(self.mistake(&message));
        };
        self.at += 1;
        let mut args = Args::new();
        if self.eat("(") {
            let mut position = 0;
            while !self.is(")") {
                // `name = value` where the query has a parameter of that name.
                let named = match (self.peek(), self.tokens.get(self.at + 1)) {
                    (Some(Token::Word(word)), Some((Token::Symbol("="), _))) => overloads
                        .iter()
                        .flat_map(|overload| &overload.params)
                        .find(|param| *param.name == **word)
                        .map(|param| param.name.clone()),
                    _ => None,
                };
                let param = match named {
                    Some(param) => {
                        self.at += 2;
                        param
                    }
                    None => {
                        let fitting = overloads.iter().find_map(|o| o.params.get(position));
                        let param = fitting.ok_or_else(|| {
                            self.mistake(&format!("“{}” does not take this many.", short(query)))
                        })?;
                        position += 1;
                        param.name.clone()
                    }
                };
                args.push((param, Arg::Expr(self.expr()?)));
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(")")?;
        }
        Ok(Expr::Query {
            target,
            query: query.clone(),
            args,
        })
    }

    fn number(&mut self, text: &str) -> Parsed {
        let whole = !text.contains(['.', 'e', 'E']);
        let parsed = match whole {
            true => text.parse::<i64>().ok().map(Expr::Int),
            false => None,
        };
        let parsed = parsed.or_else(|| text.parse::<f64>().ok().map(Expr::Real));
        parsed.ok_or_else(|| self.mistake("This is not a number."))
    }

    fn primary(&mut self) -> Parsed {
        let Some(token) = self.peek().cloned() else {
            return Err(self.mistake("Something is missing here."));
        };
        match token {
            Token::Number(number) => {
                self.at += 1;
                self.number(&number)
            }
            Token::Text(text) => {
                self.at += 1;
                Ok(Expr::Str(text))
            }
            Token::Colour(colour) => {
                let parsed = Rgba::parse(&colour);
                let parsed =
                    parsed.ok_or_else(|| self.mistake("A colour is # and six digits: #58c4dd."))?;
                self.at += 1;
                Ok(Expr::Color(parsed))
            }
            Token::Quoted(name) => {
                let found = self.names.find(&name);
                let found = found.ok_or_else(|| {
                    self.mistake(&format!("There is nothing called “{name}” here."))
                })?;
                self.at += 1;
                Ok(found)
            }
            Token::Symbol(open @ ("(" | "⟨")) => {
                self.at += 1;
                let close = if open == "(" { ")" } else { "⟩" };
                let first = self.expr()?;
                if !self.eat(",") {
                    self.expect(close)?;
                    return Ok(first);
                }
                let second = self.expr()?;
                let third = match self.eat(",") {
                    true => self.expr()?,
                    false => Expr::Real(0.0),
                };
                self.expect(close)?;
                Ok(Expr::Vec3(Box::new([first, second, third])))
            }
            Token::Symbol(_) => Err(self.mistake("A number, a name or a bracket belongs here.")),
            Token::Word(word) => self.named(&word),
        }
    }

    /// A word: the start of a name, a constant, a function, or `true` and `false`.
    fn named(&mut self, word: &str) -> Parsed {
        match word {
            "true" | "false" => {
                self.at += 1;
                return Ok(Expr::Bool(word == "true"));
            }
            keyword if KEYWORDS.contains(&keyword) => {
                return Err(self.mistake(&format!("“{keyword}” cannot stand here.")));
            }
            _ => {}
        }
        // Names may be several words long: the longest run of words that is a name wins.
        let run: Vec<&str> = self.tokens[self.at..]
            .iter()
            .map_while(|(token, _)| match token {
                Token::Word(word) if !KEYWORDS.contains(&word.as_str()) => Some(word.as_str()),
                Token::Number(number) if !number.contains(['.', 'e', 'E']) => Some(number),
                _ => None,
            })
            .collect();
        let several = (2..=run.len())
            .rev()
            .find_map(|count| Some((count, self.names.find(&run[..count].join(" "))?)));
        if let Some((count, found)) = several {
            self.at += count;
            return Ok(found);
        }
        let sig = self.names.sig;
        let called = matches!(self.tokens.get(self.at + 1), Some((Token::Symbol("("), _)));
        if called && sig.fns.contains_key(word) {
            self.at += 2;
            let mut args = Vec::new();
            while !self.is(")") {
                args.push(self.expr()?);
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(")")?;
            return Ok(Expr::call(word, args));
        }
        if sig.consts.contains_key(word) {
            self.at += 1;
            return Ok(Expr::konst(word));
        }
        if let Some(found) = self.names.find(word) {
            self.at += 1;
            return Ok(found);
        }
        Err(self.mistake(&match called {
            true => format!("There is no function called “{word}”."),
            false => format!("There is nothing called “{word}” here."),
        }))
    }
}

/// A query's name as it is written after a point: `center` for `get_center`.
fn short(query: &str) -> &str {
    query.strip_prefix("get_").unwrap_or(query)
}

impl Names<'_> {
    fn name(&self, var: VarId) -> Option<&str> {
        self.deck.names.get(&var).map(String::as_str)
    }

    /// What `name` stands for: the innermost value of that name, else the newest object.
    fn find(&self, name: &str) -> Option<Expr> {
        let called = |var: &&VarId| self.name(**var) == Some(name);
        let value = self.values.iter().rev().find(called);
        let object = self.objects.iter().rev().find(called);
        value
            .map(|var| Expr::Var(*var))
            .or(object.map(|var| Expr::Place(Place::var(*var))))
    }

    /// A name as it is written in a formula: bare if that reads back as the same name, in
    /// backticks otherwise.
    fn written(&self, var: VarId) -> String {
        let Some(name) = self.name(var) else {
            return "?".to_owned();
        };
        let words: Vec<&str> = name.split(' ').collect();
        let wordlike = |word: &&str| {
            let mut chars = word.chars();
            let first = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
            first && chars.all(|c| c.is_alphanumeric() || c == '_')
        };
        let numeral = |word: &&str| !word.is_empty() && word.chars().all(|c| c.is_ascii_digit());
        let plain = words.first().is_some_and(wordlike)
            && words.iter().all(|word| wordlike(word) || numeral(word))
            && !words.iter().any(|word| KEYWORDS.contains(word));
        // A function's name is only read as the function where a bracket follows, which
        // never follows a name; so `x` can be a value although `p.x` is a part of a point.
        let taken = self.sig.consts.contains_key(name);
        match plain && !taken {
            true => name.to_owned(),
            false => format!("`{name}`"),
        }
    }
}

// ── Stretches ────────────────────────────────────────────────────────────────

/// Reads stretches of the number line as they are written by hand: `[-1, -0.1] ∪ [0.1, 1]`.
/// The two ends of each are formulas, parted by a comma or a semicolon; the stretches are
/// joined by `∪`, `U`, `union` or `or`, or just stand side by side. Nothing at all is no
/// stretch at all.
pub fn parse_stretches(text: &str, names: &Names) -> Result<Vec<(Expr, Expr)>, Mistake> {
    let chars: Vec<char> = text.chars().collect();
    let mistake = |at: usize, message: &str| Mistake {
        at,
        message: message.to_owned(),
    };
    let example = "Write stretches like [-1, -0.1] ∪ [0.1, 1].";
    let mut stretches = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() || c == '∪' || c == ',' {
            at += 1;
            continue;
        }
        if c.is_alphabetic() {
            let end = (at..chars.len()).find(|&index| !chars[index].is_alphabetic());
            let end = end.unwrap_or(chars.len());
            let word: String = chars[at..end].iter().collect();
            if !["u", "union", "or", "and"].contains(&word.to_lowercase().as_str()) {
                return Err(mistake(at, example));
            }
            at = end;
            continue;
        }
        if c != '[' {
            return Err(mistake(at, example));
        }
        // To the bracket that closes this one, past any brackets inside the formulas.
        let (mut depth, mut parts, mut close) = (0, vec![at + 1], None);
        for (index, &inner) in chars.iter().enumerate().skip(at + 1) {
            match inner {
                '(' | '[' | '⟨' => depth += 1,
                ')' | '⟩' => depth -= 1,
                ']' if depth > 0 => depth -= 1,
                ']' => {
                    close = Some(index);
                    break;
                }
                ';' | ',' if depth == 0 => parts.push(index + 1),
                _ => {}
            }
        }
        let close = close.ok_or_else(|| mistake(at, "This stretch has no closing ]."))?;
        let &[first, second] = parts.as_slice() else {
            return Err(mistake(at, "A stretch has two ends: [from, to]."));
        };
        let read = |from: usize, to: usize| {
            let part: String = chars[from..to].iter().collect();
            parse(&part, names).map_err(|inner| mistake(from + inner.at, &inner.message))
        };
        stretches.push((read(first, second - 1)?, read(second, close)?));
        at = close + 1;
    }
    Ok(stretches)
}

/// Writes stretches so that [`parse_stretches`] reads them back.
pub fn write_stretches(stretches: &[(Expr, Expr)], names: &Names) -> String {
    let written: Vec<String> = stretches
        .iter()
        .map(|(from, to)| format!("[{}, {}]", write(from, names), write(to, names)))
        .collect();
    written.join(" ∪ ")
}

// ── Writing ──────────────────────────────────────────────────────────────────

/// How tightly each form binds; a form is put in brackets where a tighter one is expected.
mod level {
    pub const IF: u8 = 0;
    pub const OR: u8 = 1;
    pub const AND: u8 = 2;
    pub const NOT: u8 = 3;
    pub const COMPARISON: u8 = 4;
    pub const SUM: u8 = 5;
    pub const PRODUCT: u8 = 6;
    pub const SIGNED: u8 = 7;
    pub const POWER: u8 = 8;
    pub const WHOLE: u8 = 9;
}

/// Writes `expr` as a formula that [`parse`] reads back as `expr`.
pub fn write(expr: &Expr, names: &Names) -> String {
    names.at_least(expr, level::IF)
}

impl Names<'_> {
    fn at_least(&self, expr: &Expr, wanted: u8) -> String {
        let (text, level) = self.leveled(expr);
        match level >= wanted {
            true => text,
            false => format!("({text})"),
        }
    }

    fn place(&self, place: &Place) -> String {
        let path: String = place
            .path
            .iter()
            .map(|index| format!("[{index}]"))
            .collect();
        format!("{}{path}", self.written(place.root))
    }

    fn leveled(&self, expr: &Expr) -> (String, u8) {
        use level::*;
        let numeral = |text: String| {
            let level = if text.starts_with('-') { SIGNED } else { WHOLE };
            (text, level)
        };
        match expr {
            Expr::Real(value) => numeral(format!("{value:?}")),
            Expr::Int(value) => numeral(value.to_string()),
            Expr::Bool(value) => (value.to_string(), WHOLE),
            Expr::Str(value) => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                (format!("\"{escaped}\""), WHOLE)
            }
            Expr::Vec3(parts) => {
                let [x, y, z] = &**parts;
                let flat = matches!(z, Expr::Real(z) if *z == 0.0 && z.is_sign_positive());
                let shown = if flat { &parts[..2] } else { &parts[..] };
                let _ = (x, y);
                let parts: Vec<String> = shown.iter().map(|part| self.at_least(part, IF)).collect();
                (format!("({})", parts.join(", ")), WHOLE)
            }
            Expr::Color(color) => (color.hex(), WHOLE),
            Expr::Const(name) => (name.to_string(), WHOLE),
            Expr::Var(var) => (self.written(*var), WHOLE),
            Expr::Place(place) => (self.place(place), WHOLE),
            Expr::If {
                cond,
                then,
                otherwise,
            } => {
                let text = format!(
                    "if {} then {} else {}",
                    self.at_least(cond, OR),
                    self.at_least(then, OR),
                    self.at_least(otherwise, IF)
                );
                (text, IF)
            }
            Expr::Query {
                target,
                query,
                args,
            } => (self.query(target, query, args), WHOLE),
            Expr::Call { fun, args } => self.call(fun, args),
        }
    }

    fn query(&self, target: &Place, query: &str, args: &Args) -> String {
        let overloads = self.sig.queries.get(query);
        let first: Option<&Query> = overloads.and_then(|overloads| overloads.first());
        // `get_center()` is written `center` unless that would be read as another query.
        let brief = short(query);
        let name = match args.is_empty() && !self.sig.queries.contains_key(brief) {
            true => brief,
            false => query,
        };
        let bare = args.is_empty() && (name != query || first.is_some_and(|q| q.property));
        if bare {
            return format!("{}.{name}", self.place(target));
        }
        // By position as long as the arguments follow the parameters, by name after that.
        let params: Vec<&str> = first
            .map(|query| query.params.iter().map(|p| p.name.as_str()).collect())
            .unwrap_or_default();
        let mut in_order = true;
        let parts: Vec<String> = args
            .iter()
            .enumerate()
            .map(|(index, (param, arg))| {
                let value = match arg {
                    Arg::Expr(expr) => self.at_least(expr, level::IF),
                    _ => "?".to_owned(),
                };
                in_order &= params.get(index) == Some(&param.as_str());
                match in_order {
                    true => value,
                    false => format!("{param} = {value}"),
                }
            })
            .collect();
        format!("{}.{name}({})", self.place(target), parts.join(", "))
    }

    fn call(&self, fun: &str, args: &[Expr]) -> (String, u8) {
        use level::*;
        let form = self
            .sig
            .fns
            .get(fun)
            .and_then(|overloads| overloads.first());
        let form = form.map(|function| &function.py);
        let two = |level: u8, left: u8, right: u8, symbol: &str| {
            let text = format!(
                "{} {symbol} {}",
                self.at_least(&args[0], left),
                self.at_least(&args[1], right)
            );
            (text, level)
        };
        let numeral = |expr: &Expr| matches!(expr, Expr::Real(_) | Expr::Int(_));
        match (fun, form, args) {
            ("or", _, [_, _]) => two(OR, OR, AND, "or"),
            ("and", _, [_, _]) => two(AND, AND, NOT, "and"),
            ("not", _, [operand]) => (format!("not {}", self.at_least(operand, NOT)), NOT),
            ("==", _, [_, _]) => two(COMPARISON, SUM, SUM, "="),
            ("<" | "<=" | ">" | ">=" | "!=", _, [_, _]) => two(COMPARISON, SUM, SUM, fun),
            ("+" | "-", _, [_, _]) => two(SUM, SUM, PRODUCT, fun),
            ("*" | "/", _, [_, _]) => two(PRODUCT, PRODUCT, SIGNED, fun),
            ("pow", _, [_, _]) => two(POWER, WHOLE, SIGNED, "^"),
            // A number in brackets, so that it is not read back as that number negated.
            ("neg", _, [operand]) if numeral(operand) => {
                (format!("-({})", self.at_least(operand, IF)), SIGNED)
            }
            ("neg", _, [operand]) => (format!("-{}", self.at_least(operand, SIGNED)), SIGNED),
            // Of an object, `.x` asks the object; the part of a point is written as a call.
            (_, Some(PyForm::Index(_)), [point]) if !matches!(point, Expr::Place(_)) => {
                (format!("{}.{fun}", self.at_least(point, WHOLE)), WHOLE)
            }
            _ => {
                let args: Vec<String> = args.iter().map(|arg| self.at_least(arg, IF)).collect();
                (format!("{fun}({})", args.join(", ")), WHOLE)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use suit_core::{DeckConfig, Object};

    use super::*;

    struct World {
        deck: Deck,
        sig: Signature,
        objects: Vec<VarId>,
        values: Vec<VarId>,
    }

    impl World {
        fn new() -> World {
            let mut deck = Deck::new(DeckConfig::default());
            let mut slide = deck.add_slide("One");
            let mut objects = Vec::new();
            for (name, ctor) in [
                ("circle", "Circle"),
                ("dot", "Dot"),
                ("dot 2", "Dot"),
                ("axes", "Axes"),
                ("t", "ValueTracker"),
                ("row", "Circle"),
                ("odd name!", "Circle"),
                ("UP", "Circle"),
                ("salt and pepper", "Circle"),
            ] {
                objects.push(slide.object(name, Object::new(ctor)).root);
            }
            let values = vec![slide.var("speed"), slide.var("x")];
            World {
                deck,
                sig: Signature::builtin(),
                objects,
                values,
            }
        }

        fn names(&self) -> Names<'_> {
            Names {
                deck: &self.deck,
                sig: &self.sig,
                objects: &self.objects,
                values: &self.values,
            }
        }

        fn object(&self, name: &str) -> Place {
            let var = self.objects.iter().find(|var| self.deck.names[var] == name);
            Place::var(*var.unwrap())
        }
    }

    /// Reads `text`, and checks that it is written back as `canonical` and that this reads
    /// as the same expression.
    fn read(world: &World, text: &str, canonical: &str) -> Expr {
        let names = world.names();
        let expr = parse(text, &names).unwrap_or_else(|mistake| panic!("{text}: {mistake:?}"));
        assert_eq!(write(&expr, &names), canonical, "writing {text}");
        assert_eq!(
            parse(canonical, &names).as_ref(),
            Ok(&expr),
            "rereading {canonical}"
        );
        expr
    }

    #[test]
    fn numbers_points_and_constants() {
        let world = World::new();
        assert_eq!(read(&world, "2", "2"), Expr::Int(2));
        assert_eq!(read(&world, "2.50", "2.5"), Expr::Real(2.5));
        assert_eq!(read(&world, "-3", "-3"), Expr::Int(-3));
        assert_eq!(read(&world, "1e-3", "0.001"), Expr::Real(0.001));
        assert_eq!(read(&world, "true", "true"), Expr::Bool(true));
        assert_eq!(
            read(&world, r#""say \"hi\"""#, r#""say \"hi\"""#),
            Expr::Str("say \"hi\"".into())
        );
        assert_eq!(
            read(&world, "#58C4DD", "#58c4dd"),
            Expr::Color(Rgba::rgb(0x58, 0xc4, 0xdd))
        );
        assert_eq!(
            read(&world, "( 1,2 )", "(1, 2)"),
            Expr::Vec3(Box::new([1.into(), 2.into(), 0.0.into()]))
        );
        assert_eq!(
            read(&world, "⟨1, 2, 3⟩", "(1, 2, 3)"),
            Expr::Vec3(Box::new([1.into(), 2.into(), 3.into()]))
        );
        assert_eq!(read(&world, "PI", "PI"), Expr::konst("PI"));
        assert_eq!(
            read(&world, "2*UP", "2 * UP"),
            Expr::call("*", vec![2.into(), Expr::konst("UP")])
        );
    }

    #[test]
    fn arithmetic_binds_the_usual_way() {
        let world = World::new();
        let x = Expr::Var(world.values[1]);
        let call = Expr::call;
        assert_eq!(
            read(&world, "1 + 2 * x", "1 + 2 * x"),
            call("+", vec![1.into(), call("*", vec![2.into(), x.clone()])])
        );
        assert_eq!(
            read(&world, "(1 + 2) * x", "(1 + 2) * x"),
            call("*", vec![call("+", vec![1.into(), 2.into()]), x.clone()])
        );
        assert_eq!(
            read(&world, "1 - (2 - x)", "1 - (2 - x)"),
            call("-", vec![1.into(), call("-", vec![2.into(), x.clone()])])
        );
        assert_eq!(
            read(&world, "1 - 2 - x", "1 - 2 - x"),
            call("-", vec![call("-", vec![1.into(), 2.into()]), x.clone()])
        );
        // A minus before a number is part of the number, unless a power comes first.
        assert_eq!(
            read(&world, "-2 ^ 2", "-2 ^ 2"),
            call("neg", vec![call("pow", vec![2.into(), 2.into()])])
        );
        assert_eq!(
            read(&world, "(-2) ^ 2", "(-2) ^ 2"),
            call("pow", vec![Expr::Int(-2), 2.into()])
        );
        assert_eq!(
            read(&world, "x ^ -1", "x ^ -1"),
            call("pow", vec![x.clone(), Expr::Int(-1)])
        );
        assert_eq!(read(&world, "-(2)", "-(2)"), call("neg", vec![2.into()]));
        assert_eq!(read(&world, "-x", "-x"), call("neg", vec![x.clone()]));
        assert_eq!(
            read(&world, "sin(x) / max(x, 1)", "sin(x) / max(x, 1)"),
            call(
                "/",
                vec![
                    call("sin", vec![x.clone()]),
                    call("max", vec![x.clone(), 1.into()])
                ]
            )
        );
        assert_eq!(
            read(
                &world,
                "if x >= 0 and not x = 1 then 1 else -1",
                "if x >= 0 and not x = 1 then 1 else -1"
            ),
            Expr::If {
                cond: Box::new(call(
                    "and",
                    vec![
                        call(">=", vec![x.clone(), 0.into()]),
                        call("not", vec![call("==", vec![x.clone(), 1.into()])])
                    ]
                )),
                then: Box::new(1.into()),
                otherwise: Box::new(Expr::Int(-1)),
            }
        );
        read(
            &world,
            "1 + (if x > 0 then 1 else 2)",
            "1 + (if x > 0 then 1 else 2)",
        );
    }

    #[test]
    fn objects_by_name_and_what_they_are_asked() {
        let world = World::new();
        let place = |name: &str| Expr::Place(world.object(name));
        assert_eq!(read(&world, "circle", "circle"), place("circle"));
        assert_eq!(
            read(&world, "dot 2", "dot 2"),
            place("dot 2"),
            "the longest name wins"
        );
        assert_eq!(read(&world, "dot", "dot"), place("dot"));
        assert_eq!(
            read(&world, "`odd name!`", "`odd name!`"),
            place("odd name!")
        );
        assert_eq!(
            read(&world, "`salt and pepper`", "`salt and pepper`"),
            place("salt and pepper")
        );
        // An object that is called like a constant needs its backticks to be meant.
        assert_eq!(read(&world, "`UP`", "`UP`"), place("UP"));
        assert_eq!(read(&world, "UP", "UP"), Expr::konst("UP"));
        assert_eq!(
            read(&world, "row[2]", "row[2]"),
            Expr::Place(world.object("row").child(2))
        );

        let centre = Expr::query(&world.object("circle"), "get_center");
        assert_eq!(read(&world, "circle.center", "circle.center"), centre);
        assert_eq!(read(&world, "circle.get_center()", "circle.center"), centre);
        assert_eq!(
            read(&world, "dot 2.center.y", "dot 2.center.y"),
            Expr::call("y", vec![Expr::query(&world.object("dot 2"), "get_center")])
        );
        assert_eq!(
            read(&world, "dot.x", "dot.x"),
            Expr::query(&world.object("dot"), "get_x")
        );
        assert_eq!(
            read(&world, "circle.width", "circle.width"),
            Expr::query(&world.object("circle"), "width")
        );
        assert_eq!(read(&world, "t.value * speed", "t.value * speed"), {
            let value = Expr::query(&world.object("t"), "get_value");
            Expr::call("*", vec![value, Expr::Var(world.values[0])])
        });
        let on_axes = Expr::Query {
            target: world.object("axes"),
            query: "c2p".into(),
            args: vec![
                ("x".into(), 1.into()),
                ("y".into(), Expr::Var(world.values[1]).into()),
            ],
        };
        assert_eq!(read(&world, "axes.c2p(1, x)", "axes.c2p(1, x)"), on_axes);
        assert_eq!(
            read(&world, "axes.c2p(x = 1, y = x)", "axes.c2p(1, x)"),
            on_axes
        );
        assert_eq!(
            read(&world, "circle.corner(UR)", "circle.get_corner(UR)"),
            Expr::Query {
                target: world.object("circle"),
                query: "get_corner".into(),
                args: vec![("direction".into(), Expr::konst("UR").into())],
            }
        );
    }

    #[test]
    fn what_is_not_a_formula_says_why_and_where() {
        let world = World::new();
        let names = world.names();
        let wrong = |text: &str| parse(text, &names).expect_err(text);
        assert_eq!(wrong("").message, "Write a number, a name or a formula.");
        assert_eq!(
            wrong("1 +"),
            Mistake {
                at: 3,
                message: "Something is missing here.".into()
            }
        );
        assert_eq!(wrong("1 2").at, 2);
        assert_eq!(
            wrong("squirrel + 1").message,
            "There is nothing called “squirrel” here."
        );
        assert_eq!(
            wrong("frob(2)").message,
            "There is no function called “frob”."
        );
        assert_eq!(wrong("sin(2").message, "A “)” is missing here.");
        assert!(
            wrong("circle.smell")
                .message
                .starts_with("An object cannot be asked for “smell”.")
        );
        assert_eq!(
            wrong("(1, 2).w").message,
            "Of a point, only x, y and z can be taken."
        );
        assert_eq!(
            wrong("x[0]").message,
            "Only an object has parts to pick out."
        );
        assert_eq!(
            wrong("1 $ 2"),
            Mistake {
                at: 2,
                message: "“$” means nothing in a formula.".into()
            }
        );
        assert_eq!(
            wrong("if x then 1").message,
            "An “if” needs an “else”: what it is otherwise."
        );
        assert_eq!(
            wrong("#12").message,
            "A colour is # and six digits: #58c4dd."
        );
    }

    #[test]
    fn stretches_are_read_as_they_are_written_by_hand() {
        let world = World::new();
        let names = world.names();
        let pair = |from: Expr, to: Expr| (from, to);
        let one = vec![
            pair(Expr::Int(-1), Expr::Real(-0.1)),
            pair(Expr::Real(0.1), 1.into()),
        ];
        for text in [
            "[-1, -0.1] ∪ [0.1, 1]",
            "[-1; -0.1] union [0.1; 1]",
            "[-1,-0.1]U[0.1,1]",
            "[-1, -0.1] or [0.1, 1]",
            "[-1, -0.1] [0.1, 1]",
        ] {
            assert_eq!(parse_stretches(text, &names).as_ref(), Ok(&one), "{text}");
        }
        assert_eq!(write_stretches(&one, &names), "[-1, -0.1] ∪ [0.1, 1]");
        assert_eq!(parse_stretches("  ", &names), Ok(Vec::new()));
        // The ends are formulas like any other.
        let turn = parse_stretches("[-PI, max(1, 2) * PI]", &names).unwrap();
        assert_eq!(write_stretches(&turn, &names), "[-PI, max(1, 2) * PI]");

        let wrong = |text: &str| parse_stretches(text, &names).expect_err(text).message;
        assert_eq!(
            wrong("-1, 1"),
            "Write stretches like [-1, -0.1] ∪ [0.1, 1]."
        );
        assert_eq!(wrong("[1]"), "A stretch has two ends: [from, to].");
        assert_eq!(wrong("[1, 2"), "This stretch has no closing ].");
        assert_eq!(
            wrong("[1, squirrel]"),
            "There is nothing called “squirrel” here."
        );
    }
}
