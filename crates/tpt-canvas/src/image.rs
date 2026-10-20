//! Image handles. Decoding is out of scope; backends receive opaque references.

/// Opaque identifier for an image resource owned by the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

/// A reference to an image with known pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageRef {
    /// The image resource identifier.
    pub id: ImageId,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}
