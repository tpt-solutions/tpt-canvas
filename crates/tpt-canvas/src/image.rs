//! Image handles. Decoding is out of scope; backends receive opaque references.

/// Opaque identifier for an image resource owned by the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u32);

/// A reference to an image with known pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub id: ImageId,
    pub width: u32,
    pub height: u32,
}
