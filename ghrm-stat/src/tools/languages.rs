use crate::{Context, Row, config, language_summary};
use anyhow::Result;

pub struct Summary {
    pub total: usize,
    pub languages: Vec<Language>,
}

pub struct Language {
    pub name: String,
    pub lines: usize,
}

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let summary = language_summary(ctx)?;
    let mut rows = Vec::new();
    for language in summary.languages.iter().take(config(ctx).max_languages) {
        rows.push(Row::new(language.name.clone(), language.lines.to_string()));
    }
    if summary.total > 0 {
        rows.push(Row::new("total", summary.total.to_string()));
    }

    Ok(rows)
}

pub fn load(ctx: &Context) -> Result<Summary> {
    let mut languages = tokei::Languages::new();
    let config = tokei::Config {
        hidden: Some(config(ctx).include_hidden),
        ..tokei::Config::default()
    };
    languages.get_statistics(&[&ctx.root], &[], &config);

    let mut counts = languages
        .iter()
        .filter_map(|(kind, language)| {
            let lines = loc(kind, language);
            (lines > 0).then_some((kind.to_string(), lines))
        })
        .map(|(name, lines)| Language { name, lines })
        .collect::<Vec<_>>();
    counts.sort_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.name.cmp(&b.name)));

    Ok(Summary {
        total: counts.iter().map(|language| language.lines).sum::<usize>(),
        languages: counts,
    })
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
