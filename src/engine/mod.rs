//! Lunar's own browser engine: HTML parsing, the document model and layout
//! onto a character grid. No JavaScript, no CSS beyond hiding things, no
//! images or other subresources.

pub mod doc;
pub mod entities;
pub mod html;
pub mod layout;
