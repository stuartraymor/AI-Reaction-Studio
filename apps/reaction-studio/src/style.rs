//! The "how should it react?" choice. One pick sets perspective, intensity, writer instructions and
//! the figure's animation mood, so the main window needs no sliders. Advanced settings stay available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Calm,
    Excited,
    Skeptical,
    Amused,
    Fierce,
}
#[derive(Clone, Copy, Debug)]
pub struct ReactionStyle {
    pub id: &'static str,
    pub label: &'static str,
    pub blurb: &'static str,
    pub perspective: &'static str,
    pub intensity: u8,
    pub instructions: &'static str,
    pub mood: Mood,
}
pub const DEFAULT_STYLE: &str = "hype";
pub const STYLES: [ReactionStyle; 6] = [
    ReactionStyle {
        id: "hype",
        label: "Hype",
        blurb: "Loud, excited cheerleader",
        perspective: "Supportive",
        intensity: 80,
        instructions: "Enthusiastic and upbeat; celebrate what the source actually shows.",
        mood: Mood::Excited,
    },
    ReactionStyle {
        id: "funny",
        label: "Funny",
        blurb: "Jokes about what is on screen",
        perspective: "Comedy",
        intensity: 60,
        instructions: "Light jokes about the footage and wording; never mock people personally.",
        mood: Mood::Amused,
    },
    ReactionStyle {
        id: "skeptic",
        label: "Skeptic",
        blurb: "Questions every claim",
        perspective: "Opposing",
        intensity: 55,
        instructions: "Question claims and ask for evidence; no personal attacks.",
        mood: Mood::Skeptical,
    },
    ReactionStyle {
        id: "debate",
        label: "Debate",
        blurb: "Argues the other side",
        perspective: "Debate",
        intensity: 85,
        instructions: "Argue the strongest opposing point using the supplied evidence only.",
        mood: Mood::Fierce,
    },
    ReactionStyle {
        id: "explainer",
        label: "Explainer",
        blurb: "Calm, makes it clear",
        perspective: "Educational",
        intensity: 20,
        instructions: "Explain what is said simply, noting uncertainty.",
        mood: Mood::Calm,
    },
    ReactionStyle { id: "chill", label: "Chill", blurb: "Relaxed and neutral", perspective: "Neutral", intensity: 15, instructions: "", mood: Mood::Calm },
];
pub fn find(id: &str) -> Option<&'static ReactionStyle> {
    STYLES.iter().find(|s| s.id == id)
}
/// Unknown ids from old or hand-edited settings fall back to the default instead of failing.
pub fn find_or_default(id: &str) -> &'static ReactionStyle {
    find(id).or_else(|| find(DEFAULT_STYLE)).unwrap_or(&STYLES[0])
}
/// Mouth/brow/body motion strength 0.0..=1.0 the avatar renderer scales its keyframes by.
pub fn motion_gain(style: &ReactionStyle) -> f32 {
    gain(style.mood, style.intensity)
}
fn gain(mood: Mood, intensity: u8) -> f32 {
    let base = f32::from(intensity.min(100)) / 100.0;
    match mood {
        Mood::Calm => base.min(0.5),
        _ => base,
    }
}
/// Selected when the user changes the Advanced perspective/intensity/instructions by hand.
pub const CUSTOM: &str = "custom";
pub const MAX_EXTRA: usize = 500;

/// Picks the preset for settings saved before styles existed: same perspective, nearest intensity.
/// Perspectives no preset uses (Alternative, Custom) stay as the hand-made "custom" style.
pub fn migrate(perspective: &str, intensity: u8) -> &'static str {
    STYLES.iter().filter(|s| s.perspective == perspective).min_by_key(|s| s.intensity.abs_diff(intensity)).map(|s| s.id).unwrap_or(CUSTOM)
}
/// What the writer receives: (perspective, intensity, instructions). The optional "anything else?"
/// line is appended to the style's own instructions; Custom uses the Advanced values as typed.
pub fn resolve(style_id: &str, perspective: &str, intensity: u8, instructions: &str, extra: &str) -> (String, u8, String) {
    let extra: String = extra.trim().chars().take(MAX_EXTRA).collect();
    let (perspective, intensity, base) = match find(style_id) {
        Some(s) => (s.perspective, s.intensity, s.instructions),
        None => (perspective, intensity.min(100), instructions),
    };
    let joined = [base.trim(), extra.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" ");
    (perspective.to_string(), intensity, joined.chars().take(4000).collect())
}
/// The figure's mood and motion strength for a script; Custom/older scripts derive it from perspective.
pub fn motion(style_id: &str, perspective: &str, intensity: u8) -> (Mood, f32) {
    if let Some(s) = find(style_id) {
        return (s.mood, motion_gain(s));
    }
    let mood = match perspective {
        "Supportive" => Mood::Excited,
        "Comedy" => Mood::Amused,
        "Opposing" | "Alternative" => Mood::Skeptical,
        "Debate" => Mood::Fierce,
        _ => Mood::Calm,
    };
    (mood, gain(mood, intensity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::PERSPECTIVES;
    use std::collections::HashSet;
    #[test]
    fn ids_are_unique_and_default_exists() {
        let ids: HashSet<_> = STYLES.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), STYLES.len());
        assert!(find(DEFAULT_STYLE).is_some());
    }
    #[test]
    fn every_style_is_accepted_by_the_writer() {
        for s in &STYLES {
            assert!(PERSPECTIVES.contains(&s.perspective), "{}", s.id);
            assert!(s.intensity <= 100 && s.instructions.len() <= 4000);
            assert!(!s.label.is_empty() && !s.blurb.is_empty());
        }
    }
    #[test]
    fn styles_actually_differ_in_how_they_react() {
        assert!(STYLES.iter().map(|s| s.mood).collect::<HashSet<_>>().len() >= 4);
        assert!(STYLES.iter().map(|s| s.perspective).collect::<HashSet<_>>().len() >= 5);
        assert!(motion_gain(find_or_default("hype")) > motion_gain(find_or_default("explainer")));
    }
    #[test]
    fn unknown_style_falls_back_without_panic() {
        assert_eq!(find_or_default("nope").id, DEFAULT_STYLE);
        assert_eq!(find_or_default("").id, DEFAULT_STYLE);
        assert!(find("HYPE").is_none());
    }
    #[test]
    fn motion_gain_is_bounded() {
        for s in &STYLES {
            assert!((0.0..=1.0).contains(&motion_gain(s)));
        }
        let mut wild = STYLES[0];
        wild.intensity = 255;
        assert!(motion_gain(&wild) <= 1.0);
    }
    #[test]
    fn old_settings_migrate_to_the_nearest_preset() {
        assert_eq!(migrate("Supportive", 10), "hype");
        assert_eq!(migrate("Neutral", 30), "chill");
        assert_eq!(migrate("Educational", 90), "explainer");
        assert_eq!(migrate("Alternative", 50), CUSTOM);
        assert_eq!(migrate("Custom", 50), CUSTOM);
        assert_eq!(migrate("", 255), CUSTOM);
    }
    #[test]
    fn resolve_uses_the_preset_and_appends_the_extra_line() {
        let (p, i, text) = resolve("funny", "Neutral", 5, "ignored advanced text", "  talk like a pirate ");
        assert_eq!((p.as_str(), i), ("Comedy", 60));
        assert!(text.ends_with("talk like a pirate") && !text.contains("ignored"));
        assert_eq!(resolve("chill", "", 0, "", "").2, "");
        assert_eq!(resolve("chill", "", 0, "", "be brief").2, "be brief");
    }
    #[test]
    fn resolve_custom_keeps_advanced_values_and_caps_lengths() {
        let (p, i, text) = resolve(CUSTOM, "Alternative", 200, "Be gentle.", "");
        assert_eq!((p.as_str(), i, text.as_str()), ("Alternative", 100, "Be gentle."));
        let long = resolve(CUSTOM, "Neutral", 1, &"a".repeat(5000), &"b".repeat(5000)).2;
        assert!(long.chars().count() <= 4000);
        assert_eq!(resolve("hype", "", 0, "", &"é".repeat(900)).2.chars().filter(|c| *c == 'é').count(), MAX_EXTRA);
    }
    #[test]
    fn motion_comes_from_the_style_or_its_perspective() {
        assert_eq!(motion("skeptic", "Neutral", 0).0, Mood::Skeptical);
        assert_eq!(motion(CUSTOM, "Debate", 90), (Mood::Fierce, 0.9));
        assert_eq!(motion("", "Comedy", 40).0, Mood::Amused);
        assert!(motion("", "Neutral", 100).1 <= 0.5);
    }
}
