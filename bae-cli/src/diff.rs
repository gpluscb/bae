use similar::{ChangeTag, TextDiff};

pub fn print_diff(old: &str, new: &str) {
    // https://github.com/mitsuhiko/similar/blob/main/examples/terminal-inline.rs
    struct Line(Option<usize>);

    impl std::fmt::Display for Line {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self.0 {
                None => write!(f, "    "),
                Some(idx) => write!(f, "{:<4}", idx + 1),
            }
        }
    }

    let diff = TextDiff::from_lines(old, new);

    for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
        if idx > 0 {
            println!("{:-^1$}", "-", 80);
        }
        for op in group {
            for change in diff.iter_inline_changes(op) {
                let (sign, s) = match change.tag() {
                    ChangeTag::Delete => ("-", console::Style::new().red()),
                    ChangeTag::Insert => ("+", console::Style::new().green()),
                    ChangeTag::Equal => (" ", console::Style::new().dim()),
                };
                print!(
                    "{}{} |{}",
                    console::style(Line(change.old_index())).dim(),
                    console::style(Line(change.new_index())).dim(),
                    s.apply_to(sign).bold(),
                );
                for (emphasized, value) in change.iter_strings_lossy() {
                    if emphasized {
                        print!("{}", s.apply_to(value).underlined().on_black());
                    } else {
                        print!("{}", s.apply_to(value));
                    }
                }
                if change.missing_newline() {
                    println!();
                }
            }
        }
    }
}
