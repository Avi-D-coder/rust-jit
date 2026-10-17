//! Port of rustc's Oppen-style pretty printer (`rustc_ast_pretty/src/pp.rs` and
//! `pp/ring.rs` in the pinned rustc checkout). `stringify!` and `assert!`
//! messages are rendered by this exact algorithm in rustc, including its
//! 78-column margin, so line breaks in long outputs match the compiler.
//! The printer is pure text layout; it never inspects guest semantics.
use std::borrow::Cow;
use std::collections::VecDeque;

/// rustc `pp.rs`: `Breaks`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Breaks {
    Consistent,
    Inconsistent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IndentStyle {
    Visual,
    Block { offset: isize },
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct BreakToken {
    offset: isize,
    blank_space: isize,
    pre_break: Option<char>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BeginToken {
    indent: IndentStyle,
    breaks: Breaks,
}

#[derive(PartialEq, Eq, Debug)]
enum Token {
    String(Cow<'static, str>),
    Break(BreakToken),
    Begin(BeginToken),
    End,
}

impl Token {
    fn is_hardbreak_tok(&self) -> bool {
        *self == Printer::hardbreak_tok_offset(0)
    }
}

#[derive(Copy, Clone)]
enum PrintFrame {
    Fits,
    Broken { indent: usize, breaks: Breaks },
}

const SIZE_INFINITY: isize = 0xffff;
/// rustc `pp.rs`: target line width.
const MARGIN: isize = 78;
/// rustc `pp.rs`: every line is allowed at least this much space.
const MIN_SPACE: isize = 60;

/// rustc `pp/ring.rs`: a view onto a finite range of an infinite sequence.
struct RingBuffer<T> {
    data: VecDeque<T>,
    offset: usize,
}

impl<T> RingBuffer<T> {
    fn new() -> Self {
        RingBuffer {
            data: VecDeque::new(),
            offset: 0,
        }
    }
    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    fn push(&mut self, value: T) -> usize {
        let index = self.offset + self.data.len();
        self.data.push_back(value);
        index
    }
    fn clear(&mut self) {
        self.data.clear();
    }
    fn index_of_first(&self) -> usize {
        self.offset
    }
    fn first(&self) -> Option<&T> {
        self.data.front()
    }
    fn first_mut(&mut self) -> Option<&mut T> {
        self.data.front_mut()
    }
    fn pop_first(&mut self) -> Option<T> {
        let first = self.data.pop_front()?;
        self.offset += 1;
        Some(first)
    }
    fn last(&self) -> Option<&T> {
        self.data.back()
    }
    fn last_mut(&mut self) -> Option<&mut T> {
        self.data.back_mut()
    }
    fn get_mut(&mut self, index: usize) -> &mut T {
        &mut self.data[index - self.offset]
    }
}

struct BufEntry {
    token: Token,
    size: isize,
}

/// Opened by `cbox`/`ibox`/`rbox`, consumed by `end`.
#[must_use]
pub(crate) struct BoxMarker;

pub(crate) struct Printer {
    out: String,
    space: isize,
    buf: RingBuffer<BufEntry>,
    left_total: isize,
    right_total: isize,
    scan_stack: VecDeque<usize>,
    print_stack: Vec<PrintFrame>,
    indent: usize,
    pending_indentation: isize,
    last_printed: Option<Token>,
}

impl Printer {
    pub(crate) fn new() -> Self {
        Printer {
            out: String::new(),
            space: MARGIN,
            buf: RingBuffer::new(),
            left_total: 0,
            right_total: 0,
            scan_stack: VecDeque::new(),
            print_stack: Vec::new(),
            indent: 0,
            pending_indentation: 0,
            last_printed: None,
        }
    }

    fn last_token(&self) -> Option<&Token> {
        self.last_token_still_buffered()
            .or(self.last_printed.as_ref())
    }

    fn last_token_still_buffered(&self) -> Option<&Token> {
        self.buf.last().map(|last| &last.token)
    }

    fn replace_last_token_still_buffered(&mut self, token: Token) {
        if let Some(last) = self.buf.last_mut() {
            last.token = token;
        }
    }

    fn scan_eof(&mut self) {
        if !self.scan_stack.is_empty() {
            self.check_stack(0);
            self.advance_left();
        }
    }

    fn scan_begin(&mut self, token: BeginToken) -> BoxMarker {
        if self.scan_stack.is_empty() {
            self.left_total = 1;
            self.right_total = 1;
            self.buf.clear();
        }
        let right = self.buf.push(BufEntry {
            token: Token::Begin(token),
            size: -self.right_total,
        });
        self.scan_stack.push_back(right);
        BoxMarker
    }

    fn scan_end(&mut self, _b: BoxMarker) {
        if self.scan_stack.is_empty() {
            self.print_end();
        } else {
            let right = self.buf.push(BufEntry {
                token: Token::End,
                size: -1,
            });
            self.scan_stack.push_back(right);
        }
    }

    fn scan_break(&mut self, token: BreakToken) {
        if self.scan_stack.is_empty() {
            self.left_total = 1;
            self.right_total = 1;
            self.buf.clear();
        } else {
            self.check_stack(0);
        }
        let right = self.buf.push(BufEntry {
            token: Token::Break(token),
            size: -self.right_total,
        });
        self.scan_stack.push_back(right);
        self.right_total += token.blank_space;
    }

    fn scan_string(&mut self, string: Cow<'static, str>) {
        if self.scan_stack.is_empty() {
            self.print_string(&string);
        } else {
            let len = string.len() as isize;
            self.buf.push(BufEntry {
                token: Token::String(string),
                size: len,
            });
            self.right_total += len;
            self.check_stream();
        }
    }

    /// rustc `Printer::offset`: adjust the offset of the most recent break.
    pub(crate) fn offset(&mut self, offset: isize) {
        if let Some(BufEntry {
            token: Token::Break(token),
            ..
        }) = self.buf.last_mut()
        {
            token.offset += offset;
        }
    }

    fn check_stream(&mut self) {
        while self.right_total - self.left_total > self.space {
            if self.scan_stack.front().copied() == Some(self.buf.index_of_first()) {
                self.scan_stack.pop_front();
                if let Some(first) = self.buf.first_mut() {
                    first.size = SIZE_INFINITY;
                }
            }
            self.advance_left();
            if self.buf.is_empty() {
                break;
            }
        }
    }

    fn advance_left(&mut self) {
        while self.buf.first().is_some_and(|first| first.size >= 0) {
            let Some(left) = self.buf.pop_first() else {
                break;
            };
            match &left.token {
                Token::String(string) => {
                    self.left_total += string.len() as isize;
                    self.print_string(string);
                }
                Token::Break(token) => {
                    self.left_total += token.blank_space;
                    self.print_break(*token, left.size);
                }
                Token::Begin(token) => self.print_begin(*token, left.size),
                Token::End => self.print_end(),
            }
            self.last_printed = Some(left.token);
            if self.buf.is_empty() {
                break;
            }
        }
    }

    fn check_stack(&mut self, mut depth: usize) {
        while let Some(&index) = self.scan_stack.back() {
            let right_total = self.right_total;
            let entry = self.buf.get_mut(index);
            match entry.token {
                Token::Begin(_) => {
                    if depth == 0 {
                        break;
                    }
                    self.scan_stack.pop_back();
                    entry.size += right_total;
                    depth -= 1;
                }
                Token::End => {
                    self.scan_stack.pop_back();
                    entry.size = 1;
                    depth += 1;
                }
                _ => {
                    self.scan_stack.pop_back();
                    entry.size += right_total;
                    if depth == 0 {
                        break;
                    }
                }
            }
        }
    }

    fn get_top(&self) -> PrintFrame {
        self.print_stack
            .last()
            .copied()
            .unwrap_or(PrintFrame::Broken {
                indent: 0,
                breaks: Breaks::Inconsistent,
            })
    }

    fn print_begin(&mut self, token: BeginToken, size: isize) {
        if size > self.space {
            self.print_stack.push(PrintFrame::Broken {
                indent: self.indent,
                breaks: token.breaks,
            });
            self.indent = match token.indent {
                IndentStyle::Block { offset } => {
                    usize::try_from(self.indent as isize + offset).unwrap_or(0)
                }
                IndentStyle::Visual => (MARGIN - self.space).max(0) as usize,
            };
        } else {
            self.print_stack.push(PrintFrame::Fits);
        }
    }

    fn print_end(&mut self) {
        if let Some(PrintFrame::Broken { indent, .. }) = self.print_stack.pop() {
            self.indent = indent;
        }
    }

    fn print_break(&mut self, token: BreakToken, size: isize) {
        let fits = match self.get_top() {
            PrintFrame::Fits => true,
            PrintFrame::Broken {
                breaks: Breaks::Consistent,
                ..
            } => false,
            PrintFrame::Broken {
                breaks: Breaks::Inconsistent,
                ..
            } => size <= self.space,
        };
        if fits {
            self.pending_indentation += token.blank_space;
            self.space -= token.blank_space;
        } else {
            if let Some(pre_break) = token.pre_break {
                self.out.push(pre_break);
            }
            self.out.push('\n');
            let indent = self.indent as isize + token.offset;
            self.pending_indentation = indent;
            self.space = std::cmp::max(MARGIN - indent, MIN_SPACE);
        }
    }

    fn print_string(&mut self, string: &str) {
        for _ in 0..self.pending_indentation.max(0) {
            self.out.push(' ');
        }
        self.pending_indentation = 0;
        self.out.push_str(string);
        self.space -= string.len() as isize;
    }

    pub(crate) fn rbox(&mut self, indent: isize, breaks: Breaks) -> BoxMarker {
        self.scan_begin(BeginToken {
            indent: IndentStyle::Block { offset: indent },
            breaks,
        })
    }

    pub(crate) fn ibox(&mut self, indent: isize) -> BoxMarker {
        self.rbox(indent, Breaks::Inconsistent)
    }

    pub(crate) fn cbox(&mut self, indent: isize) -> BoxMarker {
        self.rbox(indent, Breaks::Consistent)
    }

    #[allow(dead_code)]
    pub(crate) fn visual_align(&mut self) -> BoxMarker {
        self.scan_begin(BeginToken {
            indent: IndentStyle::Visual,
            breaks: Breaks::Consistent,
        })
    }

    pub(crate) fn break_offset(&mut self, n: usize, off: isize) {
        self.scan_break(BreakToken {
            offset: off,
            blank_space: n as isize,
            ..BreakToken::default()
        });
    }

    pub(crate) fn end(&mut self, b: BoxMarker) {
        self.scan_end(b)
    }

    pub(crate) fn eof(mut self) -> String {
        self.scan_eof();
        self.out
    }

    pub(crate) fn word<S: Into<Cow<'static, str>>>(&mut self, wrd: S) {
        let string = wrd.into();
        self.scan_string(string)
    }

    pub(crate) fn word_space<W: Into<Cow<'static, str>>>(&mut self, w: W) {
        self.word(w);
        self.space();
    }

    pub(crate) fn nbsp(&mut self) {
        self.word(" ")
    }

    pub(crate) fn word_nbsp<S: Into<Cow<'static, str>>>(&mut self, w: S) {
        self.word(w);
        self.nbsp()
    }

    fn spaces(&mut self, n: usize) {
        self.break_offset(n, 0)
    }

    #[allow(dead_code)]
    pub(crate) fn zerobreak(&mut self) {
        self.spaces(0)
    }

    pub(crate) fn space(&mut self) {
        self.spaces(1)
    }

    pub(crate) fn popen(&mut self) {
        self.word("(");
    }

    pub(crate) fn pclose(&mut self) {
        self.word(")");
    }

    #[allow(dead_code)]
    pub(crate) fn hardbreak(&mut self) {
        self.spaces(SIZE_INFINITY as usize)
    }

    pub(crate) fn is_beginning_of_line(&self) -> bool {
        match self.last_token() {
            Some(last_token) => last_token.is_hardbreak_tok(),
            None => true,
        }
    }

    pub(crate) fn space_if_not_bol(&mut self) {
        if !self.is_beginning_of_line() {
            self.space();
        }
    }

    fn hardbreak_tok_offset(off: isize) -> Token {
        Token::Break(BreakToken {
            offset: off,
            blank_space: SIZE_INFINITY,
            ..BreakToken::default()
        })
    }

    /// rustc `PrintState::break_offset_if_not_bol`.
    pub(crate) fn break_offset_if_not_bol(&mut self, n: usize, off: isize) {
        if !self.is_beginning_of_line() {
            self.break_offset(n, off)
        } else if off != 0
            && self
                .last_token_still_buffered()
                .is_some_and(Token::is_hardbreak_tok)
        {
            self.replace_last_token_still_buffered(Self::hardbreak_tok_offset(off));
        }
    }

    pub(crate) fn trailing_comma_or_space(&mut self) {
        self.scan_break(BreakToken {
            blank_space: 1,
            pre_break: Some(','),
            ..BreakToken::default()
        });
    }
}
