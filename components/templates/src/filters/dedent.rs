use std::borrow::Cow;

use tera::{Error, Filter, Kwargs, State, TeraResult};

#[derive(Debug, Default)]
pub struct DedentFilter {}

impl Filter<&str, TeraResult<String>> for DedentFilter {
    fn call(&self, val: &str, kwargs: Kwargs, _: &State) -> TeraResult<String> {
        let width = kwargs.get::<usize>("width")?;
        let dedent_first_line = kwargs.get::<bool>("first")?.unwrap_or(false);
        let tabstop = kwargs.get::<usize>("tabstop")?.unwrap_or(4);

        if tabstop == 0 {
            return Err(Error::message("The parameter tabstop may not be zero"));
        }

        // Determine detent amount (in spaces) if it is not specified.
        let dedent_amnt = if let Some(width) = width {
            width
        } else if dedent_first_line {
            // If `dedent_first_line` is true, the minimum common indent of all non-
            // whitespace lines is used.
            let mut dedent_amnt: Option<usize> = None;
            for line in val.lines() {
                if !line.trim().is_empty() {
                    let indent = measure_indent(line, tabstop);
                    dedent_amnt = Some(
                        dedent_amnt.map_or(indent, |dedent_amnt| usize::min(dedent_amnt, indent)),
                    );
                }
            }

            if let Some(dedent_amnt) = dedent_amnt {
                dedent_amnt
            } else {
                // Cannot determine amount/no lines dedent: return input.
                return Ok(val.to_owned());
            }
        } else {
            // If `dedent_first_line` is false, the minimum common indent of all non-
            // whitespace lines following the first line minus the indent of the first line
            // is used, pushing them inline with the first line.
            let mut lines = val.lines();

            let first_line_indent: usize;
            if let Some(first_line) = lines.next() {
                first_line_indent = measure_indent(first_line, tabstop);
            } else {
                // No lines - return input
                return Ok(val.to_owned());
            }

            let mut body_indent: Option<usize> = None;
            for line in lines {
                if !line.trim().is_empty() {
                    let indent = measure_indent(line, tabstop);
                    body_indent = Some(
                        body_indent.map_or(indent, |dedent_amnt| usize::min(dedent_amnt, indent)),
                    );
                }
            }

            if let Some(body_indent) = body_indent {
                if body_indent > first_line_indent {
                    body_indent - first_line_indent
                } else {
                    // body already less indented than first line: return input.
                    return Ok(val.to_owned());
                }
            } else {
                // Cannot determine amount/no lines dedent: return input.
                return Ok(val.to_owned());
            }
        };

        if dedent_amnt == 0 {
            // Nothing to do.
            return Ok(val.to_owned());
        }

        let mut res = String::with_capacity(val.len());

        let mut first_line = true;
        for line in val.lines() {
            if first_line {
                first_line = false;
                if dedent_first_line {
                    res.push_str(&dedent_line(line, dedent_amnt, tabstop));
                } else {
                    res.push_str(line);
                }
            } else {
                res.push('\n');
                res.push_str(&dedent_line(line, dedent_amnt, tabstop));
            }
        }

        Ok(res)
    }
}

fn measure_indent(line: &str, tabstop: usize) -> usize {
    let mut indent: usize = 0;
    for char in line.chars() {
        match char {
            ' ' => {
                indent += 1;
            }
            '\t' => {
                indent += tabstop - (indent % tabstop);
            }
            _ => {
                return indent;
            }
        }
    }

    indent
}

fn dedent_line<'a>(line: &'a str, amnt: usize, tabstop: usize) -> Cow<'a, str> {
    // Find the end of the leading whitespace region.
    let ws_end = line
        .char_indices()
        .find(|(_, c)| *c != ' ' && *c != '\t')
        .map(|(i, _)| i)
        .unwrap_or(line.len());

    // If there are no tabs in the leading whitespace, we can work directly on
    // the original string without allocating.
    if !line[..ws_end].contains('\t') {
        let remove = amnt.min(ws_end);
        return Cow::Borrowed(&line[remove..]);
    }

    // Expand all leading tabs to spaces, respecting tabstops.
    let mut expanded = String::new();
    let mut col: usize = 0;
    for c in line[..ws_end].chars() {
        match c {
            ' ' => {
                expanded.push(' ');
                col += 1;
            }
            '\t' => {
                let tab_size = tabstop - (col % tabstop);
                for _ in 0..tab_size {
                    expanded.push(' ');
                }
                col += tab_size;
            }
            _ => unreachable!(),
        }
    }

    // Remove up to `amnt` spaces from the expanded indent, then append the
    // rest of the original line (past the leading whitespace).
    let remove = amnt.min(expanded.len());
    let mut result = expanded[remove..].to_string();
    result.push_str(&line[ws_end..]);
    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tera::{Context, Filter, Kwargs, State};

    #[test]
    fn test_measure_indent() {
        assert_eq!(measure_indent("", 4), 0);
        assert_eq!(measure_indent("hello", 4), 0);
        assert_eq!(measure_indent("    hello", 4), 4);
        assert_eq!(measure_indent("  hello", 4), 2);
        assert_eq!(measure_indent("\thello", 4), 4);
        assert_eq!(measure_indent("\thello", 8), 8);
        assert_eq!(measure_indent("\t\thello", 4), 8);
        assert_eq!(measure_indent("    ", 4), 4);
        assert_eq!(measure_indent("\t\t", 4), 8);
        // Two spaces then a tab: tab aligns to next multiple of 4 -> 4 total
        assert_eq!(measure_indent("  \thello", 4), 4);
        // Tab then two spaces
        assert_eq!(measure_indent("\t  hello", 4), 6);
        // Tab then two spaces then a tab: tab aligns to next multiple of 4 -> 8 total
        assert_eq!(measure_indent("\t  \thello", 4), 8);
    }

    #[test]
    fn test_dedent_line() {
        // noop:
        assert_eq!(dedent_line("hello", 0, 4), "hello");
        assert_eq!(dedent_line("", 0, 4), "");
        // basic spaces:
        assert_eq!(dedent_line("    hello", 4, 4), "hello");
        assert_eq!(dedent_line("    hello", 2, 4), "  hello");
        // basic tab:
        assert_eq!(dedent_line("\thello", 4, 4), "hello");
        // combined:
        assert_eq!(dedent_line("\t  hello", 6, 4), "hello");
        assert_eq!(dedent_line("    \thello", 8, 4), "hello");
        // less indent:
        assert_eq!(dedent_line("  hello", 4, 4), "hello");
        // split tab:
        assert_eq!(dedent_line("\thello", 2, 4), "  hello");
        assert_eq!(dedent_line("\t", 2, 4), "  ");
        assert_eq!(dedent_line("  \thello", 3, 4), " hello");
    }

    #[test]
    fn test_dedent_line_progressive() {
        let input = "\t  \t   \t\t  \thi!";
        for tabstop in 1..=8 {
            let initial_indent = measure_indent(input, tabstop);
            for amnt in 0..=initial_indent {
                let result = dedent_line(input, amnt, tabstop);
                assert_eq!(measure_indent(&result, tabstop), initial_indent - amnt);
            }
        }
    }

    #[test]
    fn test_dedent_explicit_width() {
        let dedent = |inp: &str, width: usize, first: bool| {
            let ctx = Context::new();
            let state = State::new(&ctx);
            DedentFilter {}
                .call(inp, Kwargs::from([("width", width.into()), ("first", first.into())]), &state)
                .unwrap()
        };

        // basic detenting with/without first line:
        let input = "    hello\n    world";
        assert_eq!(dedent(input, 4, false), "    hello\nworld");

        let input = "    hello\n    world";
        assert_eq!(dedent(input, 2, false), "    hello\n  world");

        let input = "    hello\n    world";
        assert_eq!(dedent(input, 4, true), "hello\nworld");

        let input = "    hello\n    world";
        assert_eq!(dedent(input, 2, false), "    hello\n  world");

        // zero width is a noop:
        let input = "    hello\n    world";
        assert_eq!(dedent(input, 0, true), "    hello\n    world");

        // tab dedenting with/without first line:
        let input = "\thello\n\tworld";
        assert_eq!(dedent(input, 4, false), "\thello\nworld");

        let input = "\thello\n\tworld";
        assert_eq!(dedent(input, 4, true), "hello\nworld");

        // basic mixed dedenting with/without first line:
        let input = "\t  \thello\n\t    world";
        assert_eq!(dedent(input, 5, false), "\t  \thello\n   world");

        let input = "\t  \thello\n\t    world";
        assert_eq!(dedent(input, 5, true), "   hello\n   world");
    }

    #[test]
    fn test_dedent_auto_first_line() {
        let dedent = |inp: &str| {
            let ctx = Context::new();
            let state = State::new(&ctx);
            DedentFilter {}.call(inp, Kwargs::from([("first", true.into())]), &state).unwrap()
        };

        // uniform indent is fully removed:
        let input = "    hello\n    world";
        assert_eq!(dedent(input,), "hello\nworld");

        // minimum indent is used:
        let input = "    hello\n      world";
        assert_eq!(dedent(input), "hello\n  world");
        let input = "      hello\n     world";
        assert_eq!(dedent(input), " hello\nworld");

        // blank lines don't affect the minimum:
        let input = "    hello\n\n    world";
        assert_eq!(dedent(input), "hello\n\nworld");

        // whitespace-only lines don't affect the minimum:
        let input = "    hello\n   \n    world";
        assert_eq!(dedent(input), "hello\n\nworld");

        // single line:
        let input = "    hello";
        assert_eq!(dedent(input), "hello");

        // no indent is a noop:
        let input = "hello\nworld";
        assert_eq!(dedent(input), "hello\nworld");

        // tab-indented:
        let input = "\thello\n\tworld";
        assert_eq!(dedent(input), "hello\nworld");

        // mixed indent, minimum is 4 (one tab):
        let input = "\thello\n\t\tworld";
        assert_eq!(dedent(input), "hello\n    world");
    }

    #[test]
    fn test_dedent_auto_no_first_line() {
        let dedent = |inp: &str| {
            let ctx = Context::new();
            let state = State::new(&ctx);
            DedentFilter {}.call(inp, Kwargs::default(), &state).unwrap()
        };

        // body more indented than first line: body aligns to first line:
        let input = "hello\n    world\n    foo";
        assert_eq!(dedent(input), "hello\nworld\nfoo");

        // body partially more indented: only excess is removed:
        let input = "  hello\n    world\n    foo";
        assert_eq!(dedent(input), "  hello\n  world\n  foo");

        // body already at same level as first line: noop:
        let input = "    hello\n    world";
        assert_eq!(dedent(input), "    hello\n    world");

        // body less indented than first line: noop:
        let input = "    hello\nworld";
        assert_eq!(dedent(input), "    hello\nworld");

        // blank lines don't affect minimum body indent:
        let input = "hello\n    world\n\n    foo";
        assert_eq!(dedent(input), "hello\nworld\n\nfoo");

        // single line: noop:
        let input = "    hello";
        assert_eq!(dedent(input), "    hello");

        // empty string: noop:
        assert_eq!(dedent(""), "");

        // tab-indented body:
        let input = "hello\n\tworld\n\tfoo";
        assert_eq!(dedent(input), "hello\nworld\nfoo");
    }
}
