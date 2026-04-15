use std::path::Path;

/// Represents different color space models used in digital imaging and printing.
///
/// A color space defines how colors are represented and interpreted in a digital
/// context. Each variant corresponds to a specific color model with distinct
/// characteristics and use cases.
///
/// # Variants
///
/// * `Srgb` - Standard RGB color space, the most common color space for web
///   and digital displays. Uses gamma correction and is device-independent.
///
/// * `Cmyk` - Cyan, Magenta, Yellow, and Key (Black) color model used primarily
///   in color printing. Subtractive color mixing system where colors are created
///   by subtracting light from white.
///
/// * `Gray` - Grayscale color space representing images using only shades of gray,
///   from black (0%) to white (100%). Each pixel contains only intensity information.
///
/// * `Rgb` - Generic RGB color space representing colors as combinations of Red,
///   Green, and Blue light. Additive color mixing system where colors are created
///   by adding light to black.
///
/// # Examples
///
/// ```no_test
/// use my_crate::ColorSpace;
///
/// let color_space = ColorSpace::Srgb;
/// assert_eq!(color_space, ColorSpace::Srgb);
///
/// match color_space {
///     ColorSpace::Srgb => println!("Standard RGB color space"),
///     ColorSpace::Cmyk => println!("CMYK color space for printing"),
///     ColorSpace::Gray => println!("Grayscale color space"),
///     ColorSpace::Rgb => println!("Generic RGB color space"),
/// }
/// ```
///
/// # Derives
///
/// This enum automatically derives:
/// * `Debug` - For debugging and display purposes
/// * `Clone` - To create copies of color space values
/// * `Copy` - To enable copy semantics (stack-only data)
/// * `PartialEq` - To compare color spaces for equality
/// * `Eq` - To enable total equality comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    Srgb,
    Cmyk,
    Gray,
    Rgb,
}

/// An ICC (International Color Consortium) profile representation.
///
/// This structure encapsulates the raw ICC profile data and its associated color space information.
/// ICC profiles are used to ensure consistent color reproduction across different devices and
/// applications by defining the color characteristics of input, display, and output devices.
///
/// # Fields
/// * `bytes` - The raw ICC profile data as a vector of bytes
/// * `color_space` - The color space associated with this ICC profile
///
/// # Examples
/// ```no_test
/// // Create an ICC profile instance
/// let profile = IccProfile {
///     bytes: vec![/* ICC profile data */],
///     color_space: ColorSpace::Rgb,
/// };
/// ```
pub struct IccProfile {
    bytes: Vec<u8>,
    color_space: ColorSpace,
}

impl IccProfile {
    /// Creates a new instance from raw byte data.
    ///
    /// This function takes a vector of bytes representing image data and attempts to
    /// detect the color space of the image. It then constructs a new instance containing
    /// both the original byte data and the detected color space information.
    ///
    /// # Arguments
    ///
    /// * `bytes` - A vector of u8 bytes containing the raw image data
    ///
    /// # Returns
    ///
    /// * `crate::Result<Self>` - A Result containing the new instance on success,
    ///   or an error if the operation fails
    ///
    /// # Example
    ///
    /// ```no_test
    /// // Assuming ImageData is the struct containing this method
    /// let image_bytes = vec![/* ... image data ... */];
    /// let image_data = ImageData::from_bytes(image_bytes)?;
    /// ```
    pub fn from_bytes(bytes: Vec<u8>) -> crate::Result<Self> {
        let color_space = detect_color_space(&bytes);
        Ok(Self { bytes, color_space })
    }

    /// Creates a new instance by reading data from a file.
    ///
    /// This function reads the entire contents of a file at the specified path
    /// and attempts to parse it into the target type using the `from_bytes` method.
    ///
    /// # Arguments
    ///
    /// * `path` - A path to the file to read. Can be any type that implements `AsRef<Path>`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` if the file was successfully read and parsed,
    /// or an error if the file could not be read or parsing failed.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * The file cannot be read (e.g., file not found, permission denied)
    /// * The `from_bytes` method fails to parse the file contents
    ///
    /// # Example
    ///
    /// ```no_test
    /// let instance = MyType::from_file("data.bin")?;
    /// ```
    pub fn from_file(path: impl AsRef<Path>) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(bytes)
    }

    /// Creates a new ICC profile instance with sRGB color space.
    ///
    /// This function initializes an sRGB color profile using the Little CMS library,
    /// extracts the ICC profile data, and wraps it in a new instance with the
    /// appropriate color space identifier.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` containing the sRGB ICC profile data and color space
    /// identifier, or an error if the profile creation or ICC data extraction fails.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The underlying LCMS library fails to create the sRGB profile
    /// - The ICC data extraction from the profile fails
    pub fn srgb() -> crate::Result<Self> {
        let profile = lcms2::Profile::new_srgb();
        let bytes = profile.icc().map_err(lcms2::Error::from)?;
        Ok(Self {
            bytes,
            color_space: ColorSpace::Srgb,
        })
    }

    /// Returns the color space associated with this image.
    ///
    /// The color space defines how colors are represented and interpreted,
    /// such as RGB, CMYK, or grayscale. This information is essential for
    /// proper color management and display rendering.
    ///
    /// # Returns
    ///
    /// A `ColorSpace` enum variant representing the color space of the image.
    ///
    /// # Examples
    ///
    /// ```no_test
    /// let img = Image::new();
    /// match img.color_space() {
    ///     ColorSpace::Rgb => println!("Image uses RGB color space"),
    ///     ColorSpace::Cmyk => println!("Image uses CMYK color space"),
    ///     ColorSpace::Grayscale => println!("Image uses grayscale color space"),
    /// }
    /// ```
    pub fn color_space(&self) -> ColorSpace {
        self.color_space
    }

    /// Returns a byte slice reference to the underlying data.
    ///
    /// This method provides access to the raw bytes stored in the object,
    /// allowing for low-level operations or interoperability with other
    /// systems that work with byte arrays.
    ///
    /// # Returns
    ///
    /// A reference to the internal byte array (`&[u8]`) containing the data.
    ///
    /// # Examples
    ///
    /// ```
    /// let data = MyStruct::new();
    /// let bytes = data.as_bytes();
    /// println!("Data length: {}", bytes.len());
    /// ```
    ///
    /// # Note
    ///
    /// The returned slice is borrowed from `self` and has the same lifetime
    /// as the original object. Modifications to the underlying data through
    /// other methods may affect the contents of the returned slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Converts the object into its underlying byte vector.
    ///
    /// This method consumes `self` and returns the owned `Vec<u8>` that was
    /// contained within the object. It is typically used when you need to
    /// extract the raw bytes from a wrapper type.
    ///
    /// # Returns
    ///
    /// Returns the owned vector of bytes (`Vec<u8>`) that this object wraps.
    ///
    /// # Examples
    ///
    /// ```no_test
    /// // Assuming we have a struct that wraps Vec<u8>
    /// let wrapper = ByteWrapper::new(vec![1, 2, 3, 4]);
    /// let bytes = wrapper.into_bytes();
    /// assert_eq!(bytes, vec![1, 2, 3, 4]);
    /// ```
    ///
    /// # Note
    ///
    /// This operation transfers ownership of the underlying bytes and consumes
    /// the original object. After calling this method, the original object
    /// cannot be used anymore.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Detects the color space from raw byte data by examining specific header bytes.
///
/// This function inspects the byte slice to determine the color space format.
/// It checks bytes 16-19 (4 bytes) for specific color space signatures.
///
/// # Arguments
///
/// * `bytes` - A slice of bytes containing image or color data
///
/// # Returns
///
/// * `ColorSpace` - The detected color space variant:
///   - `ColorSpace::Cmyk` if bytes 16-19 contain "CMYK"
///   - `ColorSpace::Gray` if bytes 16-19 contain "GRAY"
///   - `ColorSpace::Rgb` if bytes 16-19 contain "RGB" or if:
///     - The byte slice is less than 20 bytes long
///     - The signature doesn't match any known color space
///
/// # Examples
///
/// ```no_test
/// // Assuming ColorSpace enum is defined
/// let rgb_data = [0u8; 32];
/// assert_eq!(detect_color_space(&rgb_data), ColorSpace::Rgb);
///
/// let cmyk_header = b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00CMYK";
/// assert_eq!(detect_color_space(cmyk_header), ColorSpace::Cmyk);
/// ```
fn detect_color_space(bytes: &[u8]) -> ColorSpace {
    if bytes.len() < 20 {
        return ColorSpace::Rgb;
    }
    match &bytes[16..20] {
        b"CMYK" => ColorSpace::Cmyk,
        b"GRAY" => ColorSpace::Gray,
        b"RGB" => ColorSpace::Rgb,
        _ => ColorSpace::Rgb,
    }
}
