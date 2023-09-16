pub mod links;
pub mod headings;
pub use markdown;
pub use regex;

use core::ops::Range;
use markdown::unist::Position;

const fn pos_to_range(pos: &Position) -> Range<usize> {
    pos.start.offset..pos.end.offset
}

