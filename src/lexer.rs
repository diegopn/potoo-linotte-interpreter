use crate::{Diagnostic, Position};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind<'a> {
    Word(&'a str),
    Number(&'a str),
    Text(&'a str),
    Note(&'a str),
    Symbol(&'a str),
    EndLine,
    Eof,
}

#[derive(Clone, Copy)]
pub(crate) struct Token<'a> {
    pub kind: Kind<'a>,
    pub position: Position,
    pub offset: usize,
}

pub(crate) fn tokenize(source: &str) -> Result<Vec<Token<'_>>, Diagnostic> {
    let mut lexer = Lexer {
        rest: source.strip_prefix('\u{feff}').unwrap_or(source),
        position: Position { line: 1, column: 1 },
    };
    let mut tokens = Vec::new();
    while let Some(character) = lexer.peek() {
        let position = lexer.position;
        let start = lexer.rest;
        let offset = source.len() - start.len();
        let kind = match character {
            '\n' | ';' => {
                lexer.advance();
                Kind::EndLine
            }
            '\r' if !start.starts_with("\r\n") => {
                lexer.advance();
                Kind::EndLine
            }
            ch if ch.is_whitespace() => {
                lexer.advance();
                continue;
            }
            '/' if start.starts_with("//") => {
                lexer.take_while(|ch| ch != '\n' && ch != '\r');
                continue;
            }
            '/' if start.starts_with("/*") => {
                let Some(end) = start[2..].find("*/") else {
                    return Err(position.error("commentaire non fermé"));
                };
                lexer.take_bytes(end + 4);
                continue;
            }
            '"' => {
                lexer.advance();
                let value = lexer.take_while(|ch| ch != '"');
                if lexer.advance().is_none() {
                    return Err(position.error("texte non fermé"));
                }
                Kind::Text(value)
            }
            '\'' => {
                lexer.advance();
                let value = lexer.take_while(|ch| ch.is_ascii_alphabetic());
                if value.is_empty() || lexer.advance() != Some('\'') {
                    return Err(position.error("note de musique invalide"));
                }
                Kind::Note(value)
            }
            ch if ch.is_ascii_digit()
                || (ch == '.' && start.as_bytes().get(1).is_some_and(u8::is_ascii_digit)) =>
            {
                lexer.take_while(|ch| ch.is_ascii_digit());
                if lexer.peek() == Some('.') {
                    lexer.advance();
                    lexer.take_while(|ch| ch.is_ascii_digit());
                }
                if matches!(lexer.peek(), Some('e' | 'E')) {
                    lexer.advance();
                    if matches!(lexer.peek(), Some('+' | '-')) {
                        lexer.advance();
                    }
                    lexer.take_while(|ch| ch.is_ascii_digit());
                }
                Kind::Number(&start[..start.len() - lexer.rest.len()])
            }
            ch if (ch.is_alphabetic() && ch != 'µ') || ch == '_' => Kind::Word(
                lexer.take_while(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '\'' | '#')),
            ),
            '#' | '+' | '-' | '*' | '/' | '(' | ')' | '{' | '}' | '[' | ']' | '!' | '?' | '='
            | '>' | '<' | ':' | ',' | '§' | '@' | '|' | '.' | '&' | '~' | '%' | 'µ' => {
                lexer.advance();
                if matches!(character, '!' | '=' | '>' | '<' | '+' | '-')
                    && lexer.peek() == Some('=')
                    || character == ':' && lexer.peek() == Some(':')
                    || character == '<' && matches!(lexer.peek(), Some('-' | '<'))
                    || character == '>' && lexer.peek() == Some('>')
                {
                    lexer.advance();
                }
                Kind::Symbol(&start[..start.len() - lexer.rest.len()])
            }
            ch => return Err(position.error(format!("caractère non pris en charge : {ch}"))),
        };
        tokens.push(Token {
            kind,
            position,
            offset,
        });
    }
    tokens.push(Token {
        kind: Kind::Eof,
        position: lexer.position,
        offset: source.len(),
    });
    Ok(tokens)
}

struct Lexer<'a> {
    rest: &'a str,
    position: Position,
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.rest = &self.rest[ch.len_utf8()..];
        if ch == '\n' || ch == '\r' && !self.rest.starts_with('\n') {
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.column += 1;
        }
        Some(ch)
    }

    fn take_while(&mut self, predicate: impl Fn(char) -> bool) -> &'a str {
        let start = self.rest;
        while self.peek().is_some_and(&predicate) {
            self.advance();
        }
        &start[..start.len() - self.rest.len()]
    }

    fn take_bytes(&mut self, length: usize) {
        let remaining = self.rest.len() - length;
        while self.rest.len() > remaining {
            self.advance();
        }
    }
}
