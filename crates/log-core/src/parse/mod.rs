mod framing;
mod multiline;
mod normalize;
pub use framing::{Frame, Framer};
pub use multiline::Assembler;
pub use normalize::{normalize, strip_ansi};
