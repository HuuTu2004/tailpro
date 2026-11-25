pub mod decode;
mod header;
mod framing;
mod multiline;
mod normalize;
pub use framing::{Frame, Framer};
pub use multiline::Assembler;
pub use normalize::{normalize, normalize_with, raw, strip_ansi};
