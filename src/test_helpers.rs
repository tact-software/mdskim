use crate::config::HeadingsConfig;
use crate::document::Document;
use crate::parser::parse_markdown;

pub fn make_doc(md: &str) -> Document {
    parse_markdown(md, &HeadingsConfig::default(), None)
}
