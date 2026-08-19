#[cfg(feature = "content-search")]
mod content;
pub(crate) mod path;
pub(crate) mod view;

#[cfg(all(test, feature = "content-search"))]
pub(crate) use content::SearchResult;
#[cfg(feature = "content-search")]
pub(crate) use content::{SearchOpts, SearchResponse, search};
