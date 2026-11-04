//! Review state and source intent shared by the native Jacket application.

mod bulk_edit;
mod preferences;
mod review;
mod review_draft;
mod source_link;

pub use bulk_edit::{BulkEdits, BulkPreview, BulkPreviewRow, TitleCleanup};
pub use preferences::Preferences;
pub use review::Review;
pub use review_draft::{CompletionIdentity, ReviewDraft};
pub use source_link::{SourceIntent, SourceLink};
