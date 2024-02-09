use std::io::Write;

pub fn prompt(prompt: &str) -> std::io::Result<bool> {
    let mut user_input = String::new();
    Ok(loop {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(prompt.as_bytes())?;
        stdout.write_all(b" [Y/N]")?;
        stdout.flush()?;
        drop(stdout);

        std::io::stdin().read_line(&mut user_input)?;
        user_input.make_ascii_lowercase();

        match user_input.trim() {
            "y" | "yes" => break true,
            "n" | "no" => break false,
            _ => (),
        }

        user_input.clear();
    })
}
