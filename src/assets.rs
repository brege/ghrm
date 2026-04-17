use once_cell::sync::Lazy;

const GITHUB_MARKDOWN_CSS: &str = include_str!("../assets/css/github-markdown.css");
const ADMONITIONS_CSS: &str = include_str!("../assets/css/admonitions.css");
const PAGE_CSS: &str = include_str!("../assets/css/page.css");
const FEATURES_CSS: &str = include_str!("../assets/css/features.css");

pub static BUNDLE_CSS: Lazy<String> =
    Lazy::new(|| [GITHUB_MARKDOWN_CSS, ADMONITIONS_CSS, PAGE_CSS, FEATURES_CSS].join("\n"));

pub const PREVIEW_JS: &[u8] = include_bytes!("../assets/js/preview.js");

pub const FAVICON_SVG_URL: &str = "%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20512%20512%22%20fill%3D%22%232ea043%22%3E%3Cpath%20d%3D%22M240%20216V32H92a12%2012%200%200%200-12%2012v424a12%2012%200%200%200%2012%2012h328a12%2012%200%200%200%2012-12V224H248a8%208%200%200%201-8-8z%22%2F%3E%3Cpath%20d%3D%22M272%2041.69V188a4%204%200%200%200%204%204h146.31a2%202%200%200%200%201.42-3.41L275.41%2040.27a2%202%200%200%200-3.41%201.42z%22%2F%3E%3C%2Fsvg%3E";
