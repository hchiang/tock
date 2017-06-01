use returncode::ReturnCode;

/// Simple interface for using the DACC (Digital-to-Analog Converter Controller).
pub trait DacChannel {
    /// Initialize DACC with default value 0xFF and enables DACC.
    /// Returns true on success.
    fn initialize(&self) -> ReturnCode;

	/// Set the DACC value.
	/// Returns true on success.
	fn set_value(&self, value: u32) -> ReturnCode;
}

