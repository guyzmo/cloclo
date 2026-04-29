use colored::Colorize;
use rand::seq::IndexedRandom;

/// The default port for the proxy — 9393, because why not.
pub const DEFAULT_PORT: u16 = 9393;

/// Claude Francois quotes for profile switching moments.
const QUOTES: &[&str] = &[
    "Comme d'habitude, je fais semblant...",
    "Magnifique! Le profil est change!",
    "Alexandrie, Alexandra... nouveau profil!",
    "Le telephone pleure, mais le proxy repond!",
    "C'est la meme chanson, avec un autre profil...",
    "Belles, belles, belles — comme ces configurations!",
    "Danse la vie avec un nouveau profil!",
    "Aussi libre que le proxy dans le vent...",
    "Parce que ca fait chic et choc!",
    "Cette annee-la, le profil a change...",
];

/// Farewell messages for shutdown.
const FAREWELLS: &[&str] = &[
    "Salut les copains! Le proxy s'en va...",
    "Comme d'habitude... au revoir.",
    "Alexandrie, au revoir!",
    "Le telephone ne pleure plus.",
];

/// Returns a random Claude Francois quote — perfect for profile switching.
pub fn switching_quote() -> String {
    let mut rng = rand::rng();
    QUOTES.choose(&mut rng).unwrap_or(&QUOTES[0]).to_string()
}

/// Returns a farewell message for shutdown.
pub fn farewell() -> &'static str {
    let mut rng = rand::rng();
    FAREWELLS.choose(&mut rng).unwrap_or(&FAREWELLS[0])
}

/// Returns an ASCII art startup banner with port and profile info.
pub fn startup_banner(port: u16, profile: &str) -> String {
    // Inner width of the box (between the two vertical bars) is 45 chars.
    // Build each interior line as a plain 45-char string, then wrap with borders.
    let port_str = port.to_string();
    let inner_width: usize = 45;

    let title_plain = "CLOCLO — le proxy magnifique";
    let title_padding = inner_width.saturating_sub(title_plain.len());
    let title_left = title_padding / 2;
    let title_right = title_padding - title_left;
    let title_line = format!(
        "  \u{2551}{}{}{}\u{2551}",
        " ".repeat(title_left),
        "CLOCLO — le proxy magnifique".bold().cyan(),
        " ".repeat(title_right),
    );

    let port_label = "Port:    ";
    let port_value_width = inner_width.saturating_sub(port_label.len() + 4); // 4 = "  " each side
    let port_line = format!(
        "  \u{2551}  {}{:>width$}  \u{2551}",
        port_label,
        port_str.green(),
        width = port_value_width,
    );

    let profile_label = "Profile: ";
    let profile_value_width = inner_width.saturating_sub(profile_label.len() + 4);
    let profile_line = format!(
        "  \u{2551}  {}{:>width$}  \u{2551}",
        profile_label,
        profile.bold().yellow(),
        width = profile_value_width,
    );

    let top    = format!("  \u{2554}{}\u{2557}", "\u{2550}".repeat(inner_width));
    let divider = format!("  \u{2560}{}\u{2563}", "\u{2550}".repeat(inner_width));
    let bottom = format!("  \u{255a}{}\u{255d}", "\u{2550}".repeat(inner_width));

    format!(
        "\n{top}\n{title_line}\n{divider}\n{port_line}\n{profile_line}\n{bottom}\n"
    )
}
