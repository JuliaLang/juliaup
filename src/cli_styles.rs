use clap::builder::styling::{AnsiColor, Effects, Style, Styles};

pub fn get_styles() -> Styles {
    Styles::styled()
        .header(
            Style::new()
                .fg_color(Some(AnsiColor::Green.into()))
                .effects(Effects::BOLD),
        )
        .usage(
            Style::new()
                .fg_color(Some(AnsiColor::Green.into()))
                .effects(Effects::BOLD),
        )
        .literal(
            Style::new()
                .fg_color(Some(AnsiColor::Cyan.into()))
                .effects(Effects::BOLD),
        )
        .placeholder(Style::new().fg_color(Some(AnsiColor::Cyan.into())))
        .error(
            Style::new()
                .fg_color(Some(AnsiColor::Red.into()))
                .effects(Effects::BOLD),
        )
        .valid(
            Style::new()
                .fg_color(Some(AnsiColor::Cyan.into()))
                .effects(Effects::BOLD),
        )
        .invalid(
            Style::new()
                .fg_color(Some(AnsiColor::Yellow.into()))
                .effects(Effects::BOLD),
        )
        // Style the context (e.g., [default: ...], [possible values: ...])
        .context(
            Style::new()
                .fg_color(Some(AnsiColor::BrightBlack.into()))
                .effects(Effects::DIMMED),
        )
        // Style the values within context (e.g., the "yes" in [default: yes])
        .context_value(
            Style::new()
                .fg_color(Some(AnsiColor::Yellow.into()))
                .effects(Effects::BOLD),
        )
}
