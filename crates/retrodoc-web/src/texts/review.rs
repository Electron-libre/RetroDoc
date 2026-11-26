//! The fixed text of the review of a section and of its form, one value per language (WEB-77).

use retrodoc_docs::reviews::Status;

/// Why a review or an edit is outdated, in words.
#[derive(Debug)]
pub struct OutdatedTexts {
    /// Before the reasons.
    pub heading: &'static str,
    pub rewritten: &'static str,
    /// Before the files changed since.
    pub files_changed: &'static str,
}

/// The fixed text of the review of a section.
#[derive(Debug)]
pub struct ReviewTexts {
    pub heading: &'static str,
    pub not_reviewed: &'static str,
    /// Before the author of a review.
    pub by: &'static str,
    /// Validated, wrong, to review.
    pub statuses: [&'static str; 3],
    pub status_label: &'static str,
    /// The choice of the form that removes the review.
    pub no_status: &'static str,
    pub comment: &'static str,
    pub save: &'static str,
    /// The notice of a save of the form of a section (SFM-8).
    pub saved: &'static str,
    /// The action of a section that leads to its form (SFM-6).
    pub act: &'static str,
    pub outdated: OutdatedTexts,
    /// Beside the badge of an outdated review in a list.
    pub outdated_badge: &'static str,
    /// Before the number of outdated reviews on the audit page.
    pub outdated_count: &'static str,
}

impl ReviewTexts {
    /// The label of `status`.
    #[must_use]
    pub fn status(&self, status: Status) -> &'static str {
        match status {
            Status::Validated => self.statuses[0],
            Status::Wrong => self.statuses[1],
            Status::ToReview => self.statuses[2],
        }
    }

    pub const ENGLISH: Self = Self {
        heading: "Review",
        not_reviewed: "Not reviewed",
        by: "by",
        statuses: ["Validated", "Wrong", "To review"],
        status_label: "Status",
        no_status: "No review",
        comment: "Comment",
        save: "Save",
        saved: "Saved.",
        act: "Review",
        outdated: OutdatedTexts {
            heading: "Outdated review",
            rewritten: "section rewritten since the review",
            files_changed: "files changed since the review:",
        },
        outdated_badge: "outdated",
        outdated_count: "Outdated",
    };

    pub const FRENCH: Self = Self {
        heading: "Relecture",
        not_reviewed: "Non relu",
        by: "par",
        statuses: ["Validé", "Faux", "À revoir"],
        status_label: "Statut",
        no_status: "Pas de relecture",
        comment: "Commentaire",
        save: "Enregistrer",
        saved: "Enregistré.",
        act: "Relire",
        outdated: OutdatedTexts {
            heading: "Relecture périmée",
            rewritten: "section réécrite depuis la relecture",
            files_changed: "fichiers modifiés depuis la relecture :",
        },
        outdated_badge: "relecture périmée",
        outdated_count: "Périmées",
    };
}
