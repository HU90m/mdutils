pub mod headings;
pub mod links;
pub use markdown;

use core::ops::Range;
use markdown::unist::Position;

const fn pos_to_range(pos: &Position) -> Range<usize> {
    pos.start.offset..pos.end.offset
}
