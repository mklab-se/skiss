//! ASCII art banner for the rusty-tmpl CLI.

use colored::Colorize;

const LOGO: &str = r#"
  ████  █  █  ████  █████ █  █        █████ █   █ ████  █
  █  █  █  █  █       █   █  █          █   ██ ██ █  █  █
  ████  █  █  ████    █   ████  ████    █   █ █ █ ████  █
  █ █   █  █     █    █      █          █   █   █ █     █
  █  █  ████  ████    █   ████          █   █   █ █     ████"#;

/// Print the rusty-tmpl ASCII art banner.
pub fn print_banner() {
    for line in LOGO.lines() {
        println!("{}", line.bold());
    }
}

/// Print the banner with version and subtitle.
pub fn print_banner_with_version() {
    print_banner();
    println!(
        " {} {}",
        "A template for building Rust command-line tools".dimmed(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed(),
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_is_not_empty() {
        assert!(!LOGO.is_empty());
    }

    #[test]
    fn logo_has_five_visible_lines() {
        let lines: Vec<&str> = LOGO.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 5, "Logo should have 5 lines of block letters");
    }
}
