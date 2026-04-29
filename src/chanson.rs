use colored::Colorize;
use rand::seq::IndexedRandom;

/// Default proxy port — 9393.
pub const DEFAULT_PORT: u16 = 9393;

/// Actual Claude François lyrics, used as switching confirmations.
const QUOTES: &[&str] = &[
    // Comme d'habitude (1967)
    "Comme d'habitude, toute la journée...",
    // Alexandrie Alexandra (1978)
    "Alexandrie, Alexandra — tout doux, tout tranquillement tu t'en vas",
    // Le téléphone pleure (1974)
    "Ça fait longtemps que t'es parti, pourquoi tu reviens pas ?",
    // Belles ! Belles ! Belles ! (1962)
    "Toutes les filles sont belles quand on est amoureux",
    // Cette année-là (1976)
    "Et cette année-là...",
    // Le lundi au soleil (1972)
    "Le lundi au soleil, on pourrait changer les choses",
    // Magnolias for Ever (1977)
    "Magnolias for ever, for ever and ever",
    // Chanson populaire (1973)
    "Viens nous chanter ta chanson populaire",
];

/// Farewell messages on shutdown.
const FAREWELLS: &[&str] = &[
    "Salut — comme d'habitude.",
    "C'est l'heure de partir, salut les copains.",
    "Alexandrie — on se revoit bientôt.",
    "Le téléphone ne pleure plus.",
];

/// Returns a random Claude François lyric on profile switch.
pub fn switching_quote() -> String {
    let mut rng = rand::rng();
    QUOTES.choose(&mut rng).unwrap_or(&QUOTES[0]).to_string()
}

/// Returns a farewell message for shutdown.
pub fn farewell() -> &'static str {
    let mut rng = rand::rng();
    FAREWELLS.choose(&mut rng).unwrap_or(&FAREWELLS[0])
}

/// Startup banner — simple, no fake titles.
pub fn startup_banner(port: u16, profile: &str) -> String {
    format!(
        "{} — port {}, profil {}",
        "cloclo".bold(),
        port.to_string().green(),
        profile.bold().cyan(),
    )
}
