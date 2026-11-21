//! Drafting a Rule from the user's words (ADR 0013). Common phrasings ("by
//! month", "by kind", ".pdf into Papers", "screenshots into Screenshots")
//! are read here by hand, without the Grouping Models. Anything else is
//! drafted by the describing model, its reply held to a grammar of a Rule's
//! JSON so it always parses; a reply that isn't a Rule that can be saved
//! isn't guessed at. The draft is only shown in the editor: the user saves it.

use files_cleaner_core::{
    rule_problem, DateField, DateGrouping, GroupBy, NameHow, NameMatch, Rule, RuleMatch,
};
use serde::Deserialize;

use crate::card::Chat;
use crate::{kinds, Models};

/// The Rule `words` ask for, if they're a phrasing read without the models.
pub fn draft_rule(words: &str) -> Option<Rule> {
    let text = words.trim().trim_end_matches(['.', '!']).trim();
    let (matches, group_by) = into_folder(text).or_else(|| by_field(text))?;
    Some(Rule {
        enabled: true,
        words: Some(words.trim().to_string()),
        matches,
        group_by,
    })
}

/// The Rule `words` ask for: read by hand if they're a common phrasing,
/// or else drafted by `models`. `None` when the model's reply isn't a Rule
/// that can be saved; an error when the model couldn't reply.
pub fn draft_rule_with_models(
    words: &str,
    models: &mut dyn Models,
) -> Result<Option<Rule>, String> {
    if let Some(rule) = draft_rule(words) {
        return Ok(Some(rule));
    }
    let reply = models.draft(words.trim())?;
    Ok(parse(&reply, words.trim()))
}

/// What the model replies with: a Rule without its on/off flag or words.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Drafted {
    #[serde(rename = "match")]
    matches: RuleMatch,
    group_by: GroupBy,
}

/// The Rule the model's `reply` drafts from `words`, if it's one that can be saved.
fn parse(reply: &str, words: &str) -> Option<Rule> {
    let drafted: Drafted = serde_json::from_str(reply.trim()).ok()?;
    let rule = Rule {
        enabled: true,
        words: Some(words.to_string()),
        matches: drafted.matches,
        group_by: drafted.group_by,
    };
    rule_problem(&rule).is_none().then_some(rule)
}

/// Tokens a drafted Rule takes, with every field written out and a few
/// described folders; one cut short doesn't parse.
pub const MAX_NEW_TOKENS: usize = 400;

const SYSTEM: &str = "You turn a sentence about sorting files into folders into a JSON rule. Reply with the JSON object only.";

/// What the describing model is asked, as chat messages.
pub fn chat(words: &str) -> Chat {
    let user = format!(
        "Sentence: \"{words}\"\n\n\
         Fill in the rule. \"match\" says which files it is about; leave a field null or [] when the sentence doesn't say. \
         \"kind\" is one of: {kinds}. \"extensions\" are without the dot. \"name\" holds words the file name must have. \
         Sizes are in bytes (1 MB is 1000000). \"date\" is how many days ago the file was changed or added.\n\
         \"groupBy\" says where the files go: \"folder\" puts them all into one named folder; \
         \"date\" makes one folder per \"month\" or \"year\", or \"buckets\" (today, yesterday, older); \
         \"kind\" makes one folder per kind; \"meaning\" sorts them by what they are about, such as by project or subject, \
         into the folders the sentence names (with what goes in each, if it says), or [] when it names none.",
        kinds = kinds::KINDS.join(", ")
    );
    Chat {
        system: SYSTEM,
        user,
    }
}

/// The grammar a drafted Rule is held to (llama.cpp's GBNF): `Drafted` as
/// JSON, every field present in a fixed order, kinds named as the engine
/// names them, and numbers without leading zeros that fit their fields.
pub fn grammar() -> String {
    let kinds = kinds::KINDS
        .iter()
        .map(|k| format!("\"\\\"{k}\\\"\""))
        .collect::<Vec<_>>()
        .join(" | ");
    format!(
        r#"root ::= "{{" ws "\"match\":" ws match "," ws "\"groupBy\":" ws groupby ws "}}"
match ::= "{{" ws "\"kind\":" ws (kind | "null") "," ws "\"extensions\":" ws strings "," ws "\"name\":" ws (name | "null") "," ws "\"exceptNames\":" ws strings "," ws "\"minSize\":" ws (bytes | "null") "," ws "\"maxSize\":" ws (bytes | "null") "," ws "\"date\":" ws (date | "null") ws "}}"
kind ::= {kinds}
name ::= "{{" ws "\"how\":" ws ("\"contains\"" | "\"startsWith\"" | "\"endsWith\"" | "\"is\"" | "\"pattern\"") "," ws "\"words\":" ws strings ws "}}"
date ::= "{{" ws "\"field\":" ws field "," ws "\"withinDays\":" ws (days | "null") "," ws "\"olderThanDays\":" ws (days | "null") ws "}}"
groupby ::= folder | bydate | bykind | bymeaning
folder ::= "{{" ws "\"type\":" ws "\"folder\"" "," ws "\"name\":" ws string "," ws "\"date\":" ws field ws "}}"
bydate ::= "{{" ws "\"type\":" ws "\"date\"" "," ws "\"date\":" ws field "," ws "\"by\":" ws ("\"buckets\"" | "\"month\"" | "\"year\"") ws "}}"
bykind ::= "{{" ws "\"type\":" ws "\"kind\"" ws "}}"
bymeaning ::= "{{" ws "\"type\":" ws "\"meaning\"" "," ws "\"folders\":" ws "[" ws (named ("," ws named)*)? ws "]" ws "}}"
named ::= "{{" ws "\"name\":" ws string "," ws "\"description\":" ws (string | "null") ws "}}"
field ::= "\"changed\"" | "\"added\""
strings ::= "[" ws (string ("," ws string)*)? ws "]"
string ::= "\"" [^"\\\x00-\x1f]* "\""
bytes ::= "0" | [1-9] [0-9]{{0,12}}
days ::= "0" | [1-9] [0-9]{{0,4}}
ws ::= [ \n]?
"#
    )
}

/// Words that say nothing about which files: "sort my files", "put all".
const FILLER: &[&str] = &[
    "sort",
    "organise",
    "organize",
    "group",
    "put",
    "move",
    "file",
    "files",
    "my",
    "all",
    "the",
    "every",
    "everything",
    "in",
    "into",
    "folder",
    "folders",
    "please",
];

/// "X into Y": the files X names, all into the folder Y.
fn into_folder(text: &str) -> Option<(RuleMatch, GroupBy)> {
    // ASCII lowercase keeps byte offsets, so the folder keeps its own case.
    let at = text.to_ascii_lowercase().rfind(" into ")?;
    let name = folder_named(&text[at + " into ".len()..]);
    if name.is_empty() {
        return None;
    }
    let matches = subject(&text[..at])?;
    let group_by = GroupBy::Folder {
        name: name.to_string(),
        date: DateField::Changed,
    };
    Some((matches, group_by))
}

/// The folder `text` names, without the words around it: "the Papers
/// folder" and "a folder called \"Papers\"" are "Papers".
fn folder_named(text: &str) -> &str {
    let mut name = text.trim();
    loop {
        let lower = name.to_ascii_lowercase();
        let before = [
            "the ", "a ", "an ", "my ", "new ", "folder ", "called ", "named ",
        ]
        .iter()
        .find(|w| lower.starts_with(*w));
        let after = [" folder", " folders"].iter().find(|w| lower.ends_with(*w));
        name = match (before, after) {
            (Some(w), _) => name[w.len()..].trim_start(),
            (None, Some(w)) => name[..name.len() - w.len()].trim_end(),
            (None, None) => return name.trim_matches(['"', '\'', '“', '”']).trim(),
        };
    }
}

/// "[X] by month [added]": the files X names (every file, when it names
/// none) by a date, by kind, or into Today, Yesterday and Older.
fn by_field(text: &str) -> Option<(RuleMatch, GroupBy)> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("today") && lower.contains("yesterday") {
        let date = date_field(words_of(&lower), BUCKETS)?;
        let group_by = GroupBy::Date {
            date,
            by: DateGrouping::Buckets,
        };
        return Some((RuleMatch::default(), group_by));
    }
    let (subject_text, field) = match lower.rfind(" by ") {
        Some(at) => (&text[..at], &lower[at + " by ".len()..]),
        None => ("", lower.strip_prefix("by ")?),
    };
    let mut rest = words_of(field);
    let by = match rest.next()? {
        "date" | "dates" | "day" | "days" => Some(DateGrouping::Buckets),
        "month" | "months" => Some(DateGrouping::Month),
        "year" | "years" => Some(DateGrouping::Year),
        "kind" | "kinds" | "type" | "types" => None,
        _ => return None,
    };
    let matches = subject(subject_text)?;
    let group_by = match by {
        Some(by) => GroupBy::Date {
            date: date_field(rest, &[])?,
            by,
        },
        None if rest.next().is_none() => GroupBy::Kind,
        None => return None,
    };
    Some((matches, group_by))
}

/// The words of lowercase `text`, without its punctuation.
fn words_of(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// Words that may list the date buckets: "by date: today, yesterday, older".
const BUCKETS: &[&str] = &[
    "by",
    "date",
    "today",
    "yesterday",
    "this",
    "week",
    "month",
    "older",
    "and",
    "or",
];

/// The date `words` name: changed unless "added". Words other than these,
/// `also` and filler are more than this reads.
fn date_field<'a>(words: impl Iterator<Item = &'a str>, also: &[&str]) -> Option<DateField> {
    let mut field = DateField::Changed;
    for word in words {
        match word {
            "added" | "created" => field = DateField::Added,
            "changed" | "modified" => {}
            w if also.contains(&w) || FILLER.contains(&w) => {}
            _ => return None,
        }
    }
    Some(field)
}

/// What the files `text` names have: every file for none, extensions
/// (".pdf", "pdfs", "pdf and docx"), a kind ("images"), screenshots, or a
/// name with `*` in it.
fn subject(text: &str) -> Option<RuleMatch> {
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .filter(|w| !FILLER.contains(&w.to_ascii_lowercase().as_str()))
        .filter(|w| !matches!(w.to_ascii_lowercase().as_str(), "and" | "or"))
        .collect();
    let mut matches = RuleMatch::default();
    if let [word] = words.as_slice() {
        if let Some(kind) = kind_named(word) {
            matches.kind = Some(kind);
            return Some(matches);
        }
    }
    match words.as_slice() {
        [] => {}
        [word] if word.contains(['*', '?']) => {
            matches.name = Some(NameMatch {
                how: NameHow::Pattern,
                words: vec![word.to_string()],
            });
        }
        [word] if matches!(word.to_lowercase().as_str(), "screenshot" | "screenshots") => {
            matches.name = Some(NameMatch {
                how: NameHow::Contains,
                words: vec!["screenshot".into(), "screen shot".into()],
            });
        }
        words => {
            matches.extensions = words.iter().map(|w| extension(w)).collect::<Option<_>>()?;
        }
    }
    Some(matches)
}

/// The kind `word` names, as the grouping engine names it: "image" and
/// "Images" are "Images". "Other" isn't one a phrasing names.
fn kind_named(word: &str) -> Option<String> {
    let word = word.to_lowercase();
    kinds::KINDS
        .iter()
        .filter(|kind| **kind != "Other")
        .find(|kind| {
            let kind = kind.to_lowercase();
            word == kind || word == kind.trim_end_matches('s')
        })
        .map(|kind| kind.to_string())
}

/// The extension `word` names, without the dot: ".PDF" or "pdfs" is "pdf".
/// A word without a dot must be one the engine knows a kind for.
fn extension(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    if let Some(ext) = lower.strip_prefix('.') {
        let ok = !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric());
        return ok.then(|| ext.to_string());
    }
    let known = [lower.as_str(), lower.trim_end_matches('s')]
        .into_iter()
        .find(|ext| kinds::kind(ext) != "Other")
        .map(str::to_string);
    known
}
