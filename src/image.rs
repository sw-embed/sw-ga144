use std::{fmt, str::FromStr};

use crate::{ENCODING, WORD_MASK};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image(pub(crate) [u32; 64]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageError {
    pub line: usize,
    pub message: &'static str,
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ImageError {}

impl FromStr for Image {
    type Err = ImageError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut words = [ENCODING; 64];
        let mut seen = [false; 64];
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let error = |message| ImageError {
                line: index + 1,
                message,
            };
            let mut fields = line.split_whitespace();
            let address: usize = fields
                .next()
                .and_then(|field| field.parse().ok())
                .filter(|&address| address < words.len())
                .ok_or_else(|| error("expected a decimal address from 0 to 63"))?;
            let word: u32 = fields
                .next()
                .and_then(|field| field.parse().ok())
                .filter(|&word| word <= WORD_MASK)
                .ok_or_else(|| error("expected an unsigned decimal 18-bit word"))?;
            if fields.next().is_some() {
                return Err(error("expected two fields"));
            }
            if seen[address] {
                return Err(error("duplicate address"));
            }
            seen[address] = true;
            words[address] = word;
        }
        Ok(Self(words))
    }
}
