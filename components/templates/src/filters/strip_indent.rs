use tera::{Filter, Kwargs, State, TeraResult};

#[derive(Debug, Default)]
pub struct StripIndentFilter {}

impl Filter<&str, TeraResult<String>> for StripIndentFilter {
    fn call(&self, val: &str, kwargs: Kwargs, _: &State) -> TeraResult<String> {
        let strip_first_line = kwargs.get::<bool>("first")?.unwrap_or(false);

        let mut res = String::with_capacity(val.len() * 2);

        let mut first_line = true;
        for line in val.lines() {
            if first_line {
                if strip_first_line {
                    res.push_str(line.trim_start());
                } else {
                    res.push_str(line);
                }
                first_line = false
            } else {
                res.push('\n');
                res.push_str(line.trim_start());
            }
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tera::{Context, Filter, Kwargs, State};

    #[test]
    fn test_strip_indent() {
        let strip_without_first = |inp: &str| {
            let ctx = Context::new();
            let state = State::new(&ctx);
            StripIndentFilter {}.call(inp, Kwargs::default(), &state).unwrap()
        };

        let strip_with_first = |inp: &str| {
            let ctx = Context::new();
            let state = State::new(&ctx);
            StripIndentFilter {}.call(inp, Kwargs::from([("first", true.into())]), &state).unwrap()
        };

        assert_eq!(strip_without_first("hello\n    world"), "hello\nworld");
        assert_eq!(strip_without_first("hello\n  world\n  foo"), "hello\nworld\nfoo");
        assert_eq!(strip_without_first("    hello\n    world"), "    hello\nworld");

        assert_eq!(strip_with_first("    hello\n    world"), "hello\nworld");
        assert_eq!(strip_with_first("  hello"), "hello");

        // tabs:
        assert_eq!(strip_without_first("hello\n\tworld"), "hello\nworld");
        assert_eq!(strip_with_first("\thello\n\tworld"), "hello\nworld");

        // single line/empty string:
        assert_eq!(strip_without_first("    hello"), "    hello");
        assert_eq!(strip_with_first("    hello"), "hello");
        assert_eq!(strip_without_first(""), "");

        // more lines:
        assert_eq!(strip_without_first("hello\n  world\n        foo"), "hello\nworld\nfoo");
    }
}
