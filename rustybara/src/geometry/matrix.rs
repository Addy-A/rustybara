use crate::geometry::rect::Rect;

/// A 2D transformation matrix representing an affine transformation.
///
/// This matrix is used to represent 2D transformations including translation,
/// rotation, scaling, and shearing. It stores six coefficients that define
/// the transformation in the form:
///
/// ```text
/// [a  b  e]
/// [c  d  f]
/// [0  0  1]
/// ```
///
/// Where:
/// - `a`, `b`, `c`, `d` represent the linear transformation (rotation, scaling, shearing)
/// - `e`, `f` represent the translation components
///
/// The matrix can be used to transform 2D points where a point (x, y) is transformed
/// by multiplying with this matrix to produce a new point (x', y'):
///
/// ```text
/// x' = a*x + b*y + e
/// y' = c*x + d*y + f
/// ```
///
/// # Examples
///
/// ```
/// use rustybara::geometry::Matrix;
/// let matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 10.0, f: 20.0 };
/// // This represents a translation by (10, 20)
/// ```
///
/// # Derived Traits
///
/// This struct derives `Clone`, `Copy`, and `Debug` traits:
/// - `Clone`: Allows creating copies of the matrix
/// - `Copy`: Enables copy semantics instead of move semantics
/// - `Debug`: Provides debug formatting capabilities
#[derive(Clone, Copy, Debug)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    /// Creates and returns a new identity matrix.
    ///
    /// The identity matrix is a 3x3 transformation matrix that represents no transformation.
    /// When applied to any vector or point, it leaves them unchanged.
    ///
    /// # Returns
    ///
    /// A `Matrix` instance representing the identity transformation:
    /// ```text
    /// [1.0, 0.0, 0.0]
    /// [0.0, 1.0, 0.0]
    /// [0.0, 0.0, 1.0]
    /// ```
    ///
    /// # Example
    ///
    /// ```no_test
    /// let identity_matrix = Matrix::identity();
    /// ```
    pub fn identity() -> Self {
        Matrix {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Construct from a 6-element slice in PDF order [a, b, c, d, e, f]
    pub fn from_values(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Matrix { a, b, c, d, e, f }
    }

    /// Concatenate (multiply) this matrix with another matrix
    /// In PDF, `cm` operator applies the new matrix after the current one
    /// so this means: new_ctm = old_ctm * cm_matrix
    /// This is how nested transformations work in PDF graphics state and how they compound.
    pub fn concat(&self, other: &Matrix) -> Matrix {
        // Matrix multiplication for 2D affine transformations
        Matrix {
            // Refers back to our transformation equations above
            // x' = a*x + c*y + e
            // y' = b*x + d*y + f
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }
    /// Transform a single point from local space to page space
    pub fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        // Apply the matrix transformation to a point (x, y)
        (
            // Again, returns the transformed coordinates based on the matrix equations
            // x' = a*x + c*y + e
            // y' = b*x + d*y + f
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
    
    /// Transforms a rectangle by applying the transformation to all four corners
    /// and computing the axis-aligned bounding box of the result.
    ///
    /// This method takes a rectangle and transforms each of its four corners
    /// using the current transformation matrix, then calculates the smallest
    /// axis-aligned rectangle that contains all transformed corners.
    ///
    /// # Arguments
    ///
    /// * `rect` - A reference to the input rectangle to be transformed
    ///
    /// # Returns
    ///
    /// A new `Rect` representing the axis-aligned bounding box of the transformed rectangle
    ///
    /// # Example
    ///
    /// ```
    /// use rustybara::geometry::{Matrix, Rect};
    /// let transform = Matrix::identity();
    /// let original_rect = Rect { x: 0.0, y: 0.0, width: 10.0, height: 5.0 };
    /// let transformed_rect = transform.transform_rect(&original_rect);
    /// ```
    pub fn transform_rect(&self, rect: &Rect) -> Rect {
        // Transform all four corners of the rectangle and return the bounding box
        let corners = [
            self.transform_point(rect.x, rect.y),
            self.transform_point(rect.x + rect.width, rect.y),
            self.transform_point(rect.x, rect.y + rect.height),
            self.transform_point(rect.x + rect.width, rect.y + rect.height),
        ];

        let min_x = corners
            .iter()
            .map(|(x, _)| *x)
            .fold(f64::INFINITY, f64::min);
        let max_x = corners
            .iter()
            .map(|(x, _)| *x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = corners
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::INFINITY, f64::min);
        let max_y = corners
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::NEG_INFINITY, f64::max);

        Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }
}
