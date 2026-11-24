//! The fixed text of the settling of a section (`specs/section-settle.md`), apart from `texts.rs`,
//! which is over the size of a file.

/// The fixed text of the settling of a section.
#[derive(Debug)]
pub struct SettleTexts {
    /// The button of the form of a section that starts its settling (STL-1).
    pub button: &'static str,
    /// The confirmation it asks over the exchanges of the session (STL-12).
    pub confirm: &'static str,
    /// Before the section the ask page names while it settles it (STL-3).
    pub settling: &'static str,
    /// The first question of a settling as the page shows it, given the name of the section.
    pub question: fn(&str) -> String,
    /// Why a start of a settling naming another kind of section is refused (STL-11).
    pub unknown_kind: &'static str,
    /// The heading of the proposal of an exchange (STL-7).
    pub proposed: &'static str,
    /// The link of a proposal to the form of its section, filled with it.
    pub fill: &'static str,
    /// The notice of a form filled with a proposal (STL-8).
    pub filled: &'static str,
}

impl SettleTexts {
    pub const ENGLISH: Self = Self {
        button: "Settle with the agent",
        confirm:
            "The conversation of the ask page starts over. Settle this section with the agent?",
        settling: "Settling",
        question: |name| format!("Help me settle “{name}”."),
        unknown_kind: "Only a domain, a feature or a use case can be settled.",
        proposed: "Proposed verdict",
        fill: "Fill the form",
        filled:
            "Filled with the verdict the agent proposed: nothing is saved until you save the form.",
    };

    pub const FRENCH: Self = Self {
        button: "Trancher avec l’agent",
        confirm: "La conversation de la page de questions repart de zéro. Trancher cette section avec l’agent ?",
        settling: "Section à trancher :",
        question: |name| format!("Aide-moi à trancher « {name} »."),
        unknown_kind: "Seuls un domaine, une fonctionnalité ou un cas d’usage peuvent être tranchés.",
        proposed: "Verdict proposé",
        fill: "Remplir le formulaire",
        filled: "Rempli avec le verdict proposé par l’agent : rien n’est enregistré avant que vous enregistriez le formulaire.",
    };
}
