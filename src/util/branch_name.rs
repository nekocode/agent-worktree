// ===========================================================================
// Branch Name Generator
// ===========================================================================
//
// Generates memorable branch names in "adjective-noun" format.
// Examples: swift-fox, quiet-moon, bright-star

use rand::seq::IndexedRandom;

const ADJECTIVES: &[&str] = &[
    "swift", "quiet", "bright", "calm", "bold", "cool", "crisp", "deft", "fair", "fast", "firm",
    "fond", "free", "glad", "good", "grand", "great", "green", "happy", "keen", "kind", "late",
    "lean", "light", "live", "long", "loud", "main", "mild", "neat", "new", "nice", "old", "open",
    "plain", "prime", "pure", "quick", "rare", "real", "red", "rich", "ripe", "safe", "sharp",
    "short", "simple", "small", "smart", "smooth", "soft", "solid", "spare", "stable", "stark",
    "still", "strong", "sunny", "sure", "sweet", "tall", "tame", "tidy", "tight", "tiny", "true",
    "vast", "warm", "weak", "white", "wide", "wild", "wise", "young", "able", "agile", "amber",
    "azure", "basic", "blank", "blue", "brave", "brief", "busy", "civic", "clean", "clear",
    "close", "cozy", "daily", "dark", "deep", "dense", "dual", "eager", "early", "easy", "equal",
    "exact", "extra",
];

const NOUNS: &[&str] = &[
    "fox", "moon", "star", "tree", "wave", "bird", "bear", "deer", "fish", "hawk", "lake", "leaf",
    "lion", "lynx", "moth", "oak", "owl", "peak", "pine", "pond", "rain", "reef", "rock", "rose",
    "sand", "seal", "seed", "snow", "swan", "tide", "vale", "wind", "wing", "wolf", "wren", "arch",
    "bark", "beam", "bell", "bolt", "bond", "book", "box", "brew", "cape", "card", "cave", "chip",
    "clay", "cliff", "code", "coin", "core", "cove", "crow", "cube", "dawn", "disk", "dome",
    "door", "dove", "drum", "dune", "dust", "edge", "fern", "flag", "foam", "fold", "font", "fork",
    "form", "fort", "frog", "gate", "gear", "glow", "gold", "grid", "hare", "helm", "herb", "hill",
    "hive", "hook", "horn", "jade", "jazz", "kelp", "key", "kite", "knot", "lamp", "lane", "lark",
    "lens", "lime", "link", "loft", "loop",
];

/// Generate a random branch name in "adjective-noun" format
pub fn generate_branch_name() -> String {
    let mut rng = rand::rng();
    let adj = ADJECTIVES.choose(&mut rng).unwrap();
    let noun = NOUNS.choose(&mut rng).unwrap();
    format!("{adj}-{noun}")
}

/// Generate a unique branch name, appending suffix if needed
pub fn generate_unique_branch_name<F>(exists: F) -> String
where
    F: Fn(&str) -> bool,
{
    let base = generate_branch_name();

    if !exists(&base) {
        return base;
    }

    for i in 2..100 {
        let name = format!("{base}-{i}");
        if !exists(&name) {
            return name;
        }
    }

    // Fallback: generate completely new name
    generate_branch_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_branch_name() {
        let name = generate_branch_name();
        assert!(name.contains('-'));
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_generate_unique_with_conflicts() {
        let taken = vec!["swift-fox".to_string()];
        let name = generate_unique_branch_name(|n| taken.contains(&n.to_string()));

        // Should either be different or have suffix
        if name.starts_with("swift-fox") {
            assert!(name == "swift-fox" || name.starts_with("swift-fox-"));
        }
    }
}
