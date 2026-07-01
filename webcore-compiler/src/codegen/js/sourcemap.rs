//! Source Map v3 generation for compiled WebCore inline scripts.
//!
//! Implements Base64-VLQ encoding and a builder that emits a valid
//! source map JSON pointing compiled expression closures back to their
//! originating lines in the `.webc` source file.

const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode a signed integer as Base64-VLQ (LSB first, continuation bit in bit 5).
fn encode_vlq(n: i32) -> String {
    // Sign bit is the LSB in VLQ: positive n → n<<1, negative n → (-n<<1)|1
    let mut vlq = if n < 0 { ((-n) << 1) | 1 } else { n << 1 };
    let mut result = String::new();
    loop {
        let mut digit = vlq & 0x1F;
        vlq >>= 5;
        if vlq > 0 {
            digit |= 0x20;
        }
        result.push(BASE64[digit as usize] as char);
        if vlq == 0 {
            break;
        }
    }
    result
}

/// A single source map mapping entry.
pub(crate) struct Mapping {
    pub output_line: u32,
    pub output_col: u32,
    pub source_line: u32, // 0-indexed
    pub source_col: u32,  // 0-indexed
}

/// Builder for a source map v3 JSON string.
pub(crate) struct SourceMapBuilder {
    source_name: String,
    source_content: String,
    mappings: Vec<Mapping>,
}

impl SourceMapBuilder {
    pub fn new(source_name: impl Into<String>, source_content: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            source_content: source_content.into(),
            mappings: Vec::new(),
        }
    }

    pub fn add(&mut self, m: Mapping) {
        self.mappings.push(m);
    }

    /// Encode all mappings and return a source map v3 JSON string.
    pub fn build(&self) -> String {
        // Group mappings by output line
        let max_line = self
            .mappings
            .iter()
            .map(|m| m.output_line)
            .max()
            .unwrap_or(0);

        // Build a vec of vecs: segments per output line
        let mut lines: Vec<Vec<&Mapping>> = vec![Vec::new(); (max_line + 1) as usize];
        for m in &self.mappings {
            lines[m.output_line as usize].push(m);
        }

        // Encode each line; delta-encode within a line (outputCol resets per line)
        let mut vlq_lines: Vec<String> = Vec::with_capacity(lines.len());
        let mut prev_source_line: i32 = 0;
        let mut prev_source_col: i32 = 0;

        for line_segs in &lines {
            if line_segs.is_empty() {
                vlq_lines.push(String::new());
                continue;
            }
            let mut prev_output_col: i32 = 0;
            let mut segs: Vec<String> = Vec::with_capacity(line_segs.len());
            for m in line_segs {
                let d_output_col = m.output_col as i32 - prev_output_col;
                let d_source_line = m.source_line as i32 - prev_source_line;
                let d_source_col = m.source_col as i32 - prev_source_col;

                let seg = format!(
                    "{}{}{}{}",
                    encode_vlq(d_output_col),
                    encode_vlq(0), // source index is always 0 (single source file)
                    encode_vlq(d_source_line),
                    encode_vlq(d_source_col),
                );
                segs.push(seg);

                prev_output_col = m.output_col as i32;
                prev_source_line = m.source_line as i32;
                prev_source_col = m.source_col as i32;
            }
            vlq_lines.push(segs.join(","));
        }

        let mappings_str = vlq_lines.join(";");

        // Escape source content for JSON embedding
        let escaped_content = self
            .source_content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        let escaped_name = self.source_name.replace('\\', "\\\\").replace('"', "\\\"");

        format!(
            r#"{{"version":3,"sources":["{escaped_name}"],"sourcesContent":["{escaped_content}"],"names":[],"mappings":"{mappings_str}"}}"#
        )
    }
}

/// Test-only helper to expose the VLQ encoder.
#[cfg(test)]
pub(crate) fn encode_vlq_test(n: i32) -> String {
    encode_vlq(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_zero_is_a() {
        assert_eq!(encode_vlq(0), "A");
    }

    #[test]
    fn vlq_positive_one() {
        // 1 << 1 = 2, base64[2] = 'C'
        assert_eq!(encode_vlq(1), "C");
    }

    #[test]
    fn vlq_negative_one() {
        // (-(-1) << 1) | 1 = (1 << 1) | 1 = 3, base64[3] = 'D'
        assert_eq!(encode_vlq(-1), "D");
    }
}
