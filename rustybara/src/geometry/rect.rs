/// A rectangle defined by its position and dimensions.
///
/// Represents a rectangular area with its origin at the bottom-left corner
/// (following PDF coordinate system where Y axis points upwards).
/// The rectangle extends to the right and upwards from its origin point.
///
/// # Fields
/// * `x` - The x-coordinate of the left edge (minimum X value)
/// * `y` - The y-coordinate of the bottom edge (minimum Y value, uses PDF coordinate system)
/// * `width` - The horizontal size of the rectangle
/// * `height` - The vertical size of the rectangle
///
/// # Coordinate System
/// Uses the PDF coordinate system where:
/// - X-axis increases to the right
/// - Y-axis increases upward (unlike typical screen coordinates)
/// - Origin (0,0) is typically at the bottom-left of the page
///
/// # Examples
/// ```
/// use rustybara::geometry::Rect;
/// let rect = Rect { x: 10.0, y: 20.0, width: 100.0, height: 50.0 };
/// assert_eq!(rect.x, 10.0);
/// assert_eq!(rect.y, 20.0);
/// assert_eq!(rect.width, 100.0);
/// assert_eq!(rect.height, 50.0);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,      // left edge (minimum X)
    pub y: f64,      // bottom edge (minimum Y, PDF Y axis goes up/points upwards)
    pub width: f64,  // horizontal size
    pub height: f64, // vertical size
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Expands the rectangle by adding bleed space around all edges.
    ///
    /// This method creates a new rectangle that is larger than the original by
    /// extending each edge outward by the specified bleed amount. The expansion
    /// is applied uniformly to all sides.
    ///
    /// # Arguments
    ///
    /// * `bleed` - The amount of space to add around each edge. Can be negative
    ///   to shrink the rectangle instead.
    ///
    /// # Returns
    ///
    /// A new `Rect` instance with the expanded dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use rustybara::geometry::Rect;
    /// let rect = Rect { x: 10.0, y: 10.0, width: 20.0, height: 15.0 };
    /// let expanded = rect.expand(5.0);
    /// // Result: Rect { x: 5.0, y: 5.0, width: 30.0, height: 25.0 }
    /// ```
    pub fn expand(&self, bleed: f64) -> Self {
        Rect {
            x: self.x - bleed,
            y: self.y - bleed,
            width: self.width + 2.0 * bleed,
            height: self.height + 2.0 * bleed,
        }
    }

    /// Creates a new rectangle from two corner points.
    ///
    /// The rectangle is constructed by determining the minimum bounding box
    /// that contains both corner points. The origin (x, y) of the rectangle
    /// is set to the top-left corner, with width and height calculated as
    /// the absolute distances between the x and y coordinates respectively.
    ///
    /// # Arguments
    ///
    /// * `x0` - X-coordinate of the first corner
    /// * `y0` - Y-coordinate of the first corner  
    /// * `x1` - X-coordinate of the second corner
    /// * `y1` - Y-coordinate of the second corner
    ///
    /// # Returns
    ///
    /// A new `Rect` instance with normalized coordinates and dimensions
    ///
    /// # Examples
    ///
    /// ```
    /// use rustybara::geometry::Rect;
    /// let rect = Rect::from_corners(10.0, 20.0, 30.0, 40.0);
    /// assert_eq!(rect.x, 10.0);
    /// assert_eq!(rect.y, 20.0);
    /// assert_eq!(rect.width, 20.0);
    /// assert_eq!(rect.height, 20.0);
    ///
    /// // Order of corners doesn't matter
    /// let rect2 = Rect::from_corners(30.0, 40.0, 10.0, 20.0);
    /// assert_eq!(rect.x, rect2.x);
    /// assert_eq!(rect.y, rect2.y);
    /// assert_eq!(rect.width, rect2.width);
    /// assert_eq!(rect.height, rect2.height);
    /// ```
    pub fn from_corners(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Rect {
            x: x0.min(x1),
            y: y0.min(y1),
            width: (x1 - x0).abs(),
            height: (y1 - y0).abs(),
        }
    }

    /// Converts the rectangle to a PDF array format.
    ///
    /// Returns an array of four f64 values representing the rectangle in PDF coordinate system:
    /// [x, y, right, top] where:
    /// - x: The x-coordinate of the left edge
    /// - y: The y-coordinate of the bottom edge  
    /// - right: The x-coordinate of the right edge (x + width)
    /// - top: The y-coordinate of the top edge (y + height)
    ///
    /// This format is commonly used in PDF documents to define rectangular boundaries.
    pub fn to_pdf_array(&self) -> [f64; 4] {
        [self.x, self.y, self.right(), self.top()]
    }

    /// Creates a new rectangle from a PDF array format.
    ///
    /// This function takes a slice of 4 f64 values representing a rectangle in PDF
    /// array notation `[x1, y1, x2, y2]` where `(x1, y1)` is the bottom-left corner
    /// and `(x2, y2)` is the top-right corner.
    ///
    /// # Arguments
    ///
    /// * `arr` - A slice of 4 f64 values in the order [x1, y1, x2, y2]
    ///
    /// # Returns
    ///
    /// A new `Rect` instance constructed from the corner coordinates
    ///
    /// # Panics
    ///
    /// This function will panic if the input slice has fewer than 4 elements.
    pub fn from_pdf_array(arr: &[f64]) -> Self {
        Rect::from_corners(arr[0], arr[1], arr[2], arr[3])
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }
    pub fn top(&self) -> f64 {
        self.y + self.height
    }

    /// Determines if this rectangle is completely outside the specified trim area.
    ///
    /// This method checks whether the rectangle has no overlap whatsoever with the
    /// given trim rectangle by testing if it lies completely to the left, right,
    /// above, or below the trim area.
    ///
    /// # Arguments
    ///
    /// * `trim` - A reference to the Rect that defines the trim area to check against
    ///
    /// # Returns
    ///
    /// Returns `true` if this rectangle is completely outside the trim area, `false`
    /// if there is any overlap or if this rectangle is partially/completely inside
    /// the trim area.
    ///
    /// # Examples
    ///
    /// ```
    /// use rustybara::geometry::Rect;
    /// let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    /// let trim = Rect::new(20.0, 20.0, 30.0, 30.0);
    /// assert!(rect.is_outside(&trim)); // No overlap
    ///
    /// let rect2 = Rect::new(25.0, 25.0, 35.0, 35.0);
    /// assert!(!rect2.is_outside(&trim)); // Overlapping
    /// ```
    pub fn is_outside(&self, trim: &Rect) -> bool {
        // Check if the rectangle is completely outside the trim box
        self.right() <= trim.x // entirely to the left
            || self.x >= trim.right() // entirely to the right
            || self.top() <= trim.y // entirely below
            || self.y >= trim.top() // entirely above
    }
}
