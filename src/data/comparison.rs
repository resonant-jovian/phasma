/// Stub ComparisonProvider — compares two output directories.
/// Not yet implemented.
pub struct ComparisonProvider {
    pub dir_a: String,
    pub dir_b: String,
}

impl ComparisonProvider {
    pub fn new(dir_a: impl Into<String>, dir_b: impl Into<String>) -> Self {
        Self { dir_a: dir_a.into(), dir_b: dir_b.into() }
    }
}
