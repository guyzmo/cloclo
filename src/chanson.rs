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
    let title = "  CLOCLO  ".bold().white().on_blue();
    let subtitle = "Le proxy magnifique".italic();
    format!(
        r#"
╔══════════════════════════════════════════╗
║            {title}              ║
║       {subtitle}              ║
║                                          ║
║  Port:    {port:<30}  ║
║  Profile: {profile:<30}  ║
╚══════════════════════════════════════════╝
"#,
    )
}
