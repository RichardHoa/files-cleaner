use files_cleaner_core::{
    DateField, DateGrouping, GroupBy, MeaningFolder, NameHow, NameMatch, Rule, RuleMatch,
};
use files_cleaner_grouping::{
    draft_rule, draft_rule_with_models, Card, Content, Models, Question, Vector,
};

/// A describing model whose every draft is `reply`.
struct Drafting(&'static str);

impl Models for Drafting {
    fn card_model_id(&self) -> String {
        "fake-card".to_string()
    }

    fn embedding_model_id(&self) -> String {
        "fake-embedding".to_string()
    }

    fn card(&mut self, _: &Content) -> Result<Card, String> {
        Ok(Card::default())
    }

    fn place(&mut self, _: &Question) -> Result<Option<usize>, String> {
        Ok(None)
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts.iter().map(|_| vec![1.0]).collect())
    }

    fn draft(&mut self, _: &str) -> Result<String, String> {
        Ok(self.0.to_string())
    }

    fn device(&self) -> Option<String> {
        None
    }
}

#[test]
fn common_phrasings_draft_a_rule_without_the_models() {
    assert_eq!(
        draft_rule("Sort my files by month added"),
        Some(Rule {
            enabled: true,
            words: Some("Sort my files by month added".into()),
            matches: RuleMatch::default(),
            group_by: GroupBy::Date {
                date: DateField::Added,
                by: DateGrouping::Month,
            },
        })
    );
    assert_eq!(
        draft_rule("Screenshot* into Screenshots/{month}"),
        Some(Rule {
            enabled: true,
            words: Some("Screenshot* into Screenshots/{month}".into()),
            matches: RuleMatch {
                name: Some(NameMatch {
                    how: NameHow::Pattern,
                    words: vec!["Screenshot*".into()],
                }),
                ..RuleMatch::default()
            },
            group_by: GroupBy::Folder {
                name: "Screenshots/{month}".into(),
                date: DateField::Changed,
            },
        })
    );
    assert_eq!(
        draft_rule(".pdf into Papers").map(|r| r.matches.extensions),
        Some(vec!["pdf".to_string()])
    );
    let folder = |words: &str| draft_rule(words).map(|r| r.group_by);
    assert_eq!(
        folder("pdfs into the Papers folder"),
        Some(GroupBy::Folder {
            name: "Papers".into(),
            date: DateField::Changed,
        })
    );
    assert_eq!(
        folder("images into a folder called \"Tax 2025\""),
        Some(GroupBy::Folder {
            name: "Tax 2025".into(),
            date: DateField::Changed,
        })
    );
    assert_eq!(draft_rule("put my files in folders by project"), None);
}

#[test]
fn other_phrasings_are_drafted_by_the_model_and_a_bad_reply_is_never_guessed_at() {
    let words = "keep my thesis stuff and tax papers apart";
    let reply = r#"{"match": {"kind": "Documents", "extensions": [], "name": null, "exceptNames": [],
        "minSize": null, "maxSize": null, "date": null},
        "groupBy": {"type": "meaning", "folders": [{"name": "Thesis", "description": null},
        {"name": "Taxes", "description": "tax returns and receipts"}]}}"#;

    assert_eq!(
        draft_rule_with_models(words, &mut Drafting(reply)),
        Ok(Some(Rule {
            enabled: true,
            words: Some(words.into()),
            matches: RuleMatch {
                kind: Some("Documents".into()),
                ..RuleMatch::default()
            },
            group_by: GroupBy::Meaning {
                folders: vec![
                    MeaningFolder {
                        name: "Thesis".into(),
                        description: None,
                    },
                    MeaningFolder {
                        name: "Taxes".into(),
                        description: Some("tax returns and receipts".into()),
                    },
                ],
            },
        }))
    );
    // Its smallest size is above its largest: the reply doesn't validate.
    let bad = r#"{"match": {"kind": null, "extensions": [], "name": null, "exceptNames": [],
        "minSize": 5000000, "maxSize": 1000, "date": null},
        "groupBy": {"type": "kind"}}"#;
    assert_eq!(draft_rule_with_models(words, &mut Drafting(bad)), Ok(None));
    // A common phrasing never asks the model.
    assert_eq!(
        draft_rule_with_models("by kind", &mut Drafting("not json")).map(|r| r.is_some()),
        Ok(true)
    );
}
