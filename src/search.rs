mod content;
pub(crate) mod path;
pub(crate) mod view;

#[cfg(test)]
pub(crate) use content::SearchResult;
pub(crate) use content::{SearchOpts, SearchResponse, search};
