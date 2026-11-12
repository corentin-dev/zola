mod context;
mod markdown;
mod typography;

use errors::Result;

pub use context::MarkdownContext;
pub use markdown::Rendered;

pub fn render_content(content: &str, context: &MarkdownContext) -> Result<markdown::Rendered> {
    markdown::State::default().render(content, context)
}
