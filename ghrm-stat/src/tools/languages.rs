use crate::{Context, Row};
use anyhow::Result;

const LIMIT: usize = 6;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let mut languages = tokei::Languages::new();
    let config = tokei::Config {
        hidden: Some(false),
        ..tokei::Config::default()
    };
    languages.get_statistics(&[&ctx.root], &[], &config);

    let mut counts = languages
        .iter()
        .filter_map(|(kind, language)| {
            let lines = loc(kind, language);
            (lines > 0).then_some((kind.to_string(), lines))
        })
        .collect::<Vec<_>>();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total = counts.iter().map(|(_, lines)| lines).sum::<usize>();
    let mut rows = vec![Row::new("lines", total.to_string())];
    for (name, lines) in counts.into_iter().take(LIMIT) {
        let percent = if total == 0 {
            0.0
        } else {
            lines as f64 / total as f64 * 100.0
        };
        rows.push(Row::new(name, format!("{percent:.1}%")));
    }

    Ok(rows)
}

fn loc(kind: &tokei::LanguageType, language: &tokei::Language) -> usize {
    language_loc(kind, language.code, language.comments)
        + language
            .children
            .iter()
            .flat_map(|(child_kind, reports)| {
                reports.iter().map(move |report| {
                    let stats = report.stats.summarise();
                    language_loc(child_kind, stats.code, stats.comments)
                })
            })
            .sum::<usize>()
}

fn language_loc(kind: &tokei::LanguageType, code: usize, comments: usize) -> usize {
    match kind {
        tokei::LanguageType::Markdown => code + comments,
        _ => code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_counts_comments_as_loc() {
        assert_eq!(language_loc(&tokei::LanguageType::Markdown, 2, 3), 5);
    }

    #[test]
    fn rust_counts_code_only() {
        assert_eq!(language_loc(&tokei::LanguageType::Rust, 2, 3), 2);
    }
}
