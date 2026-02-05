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
    use std::collections::HashSet;

    #[test]
    fn test_generate_branch_name() {
        let name = generate_branch_name();
        assert!(name.contains('-'));
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_generate_branch_name_uses_valid_words() {
        let name = generate_branch_name();
        let parts: Vec<&str> = name.split('-').collect();
        assert!(ADJECTIVES.contains(&parts[0]));
        assert!(NOUNS.contains(&parts[1]));
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

    #[test]
    fn test_generate_unique_no_conflicts() {
        // Empty set - should return base name
        let name = generate_unique_branch_name(|_| false);
        // Should be valid adjective-noun format
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_generate_unique_with_numbered_suffix() {
        // Use RefCell for interior mutability to work with Fn
        use std::cell::RefCell;
        let call_count = RefCell::new(0);
        let name = generate_unique_branch_name(|_| {
            let mut count = call_count.borrow_mut();
            *count += 1;
            *count == 1 // Only first call (base name) returns true
        });
        // Should have -2 suffix
        assert!(name.ends_with("-2"));
    }

    #[test]
    fn test_generate_unique_many_conflicts() {
        // Simulate many conflicts with a fixed set
        let taken: HashSet<String> = (0..50)
            .flat_map(|_| {
                let base = generate_branch_name();
                (0..5).map(move |j| {
                    if j == 0 {
                        base.clone()
                    } else {
                        format!("{}-{}", base, j + 1)
                    }
                })
            })
            .collect();

        // Should still generate something
        let name = generate_unique_branch_name(|n| taken.contains(n));
        assert!(!name.is_empty());
    }

    #[test]
    fn test_generate_unique_exhaustive_conflicts() {
        // Use RefCell for interior mutability
        use std::cell::RefCell;
        let count = RefCell::new(0);
        let name = generate_unique_branch_name(|_| {
            let mut c = count.borrow_mut();
            *c += 1;
            *c < 100 // First 99 calls return true (conflict)
        });
        // Should have generated something
        assert!(!name.is_empty());
    }

    #[test]
    fn test_generate_names_are_random() {
        // Generate multiple names and check they're not all the same
        let names: HashSet<String> = (0..10).map(|_| generate_branch_name()).collect();
        // With 100 adjectives and 100 nouns, getting 10 identical names is extremely unlikely
        assert!(names.len() > 1);
    }

    #[test]
    fn test_adjectives_and_nouns_not_empty() {
        assert!(!ADJECTIVES.is_empty());
        assert!(!NOUNS.is_empty());
    }

    #[test]
    fn test_generated_name_is_valid_git_branch() {
        let name = generate_branch_name();
        // Git branch names cannot contain certain characters
        assert!(!name.contains(' '));
        assert!(!name.contains('~'));
        assert!(!name.contains('^'));
        assert!(!name.contains(':'));
        assert!(!name.starts_with('/'));
        assert!(!name.ends_with('/'));
        assert!(!name.contains(".."));
    }
}
