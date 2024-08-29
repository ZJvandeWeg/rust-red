use std::fmt;
use std::hash::Hash;
use std::io::Empty;
use std::ops::BitXor;

use crate::utils;

#[derive(
    Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde:: Deserialize,
)]
pub struct ElementId(u64);

impl BitXor for ElementId {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        ElementId(self.0 ^ rhs.0)
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl Default for ElementId {
    fn default() -> Self {
        Self::empty()
    }
}

impl ElementId {
    pub fn new() -> Self {
        Self(utils::generate_uid())
    }

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn with_u64(id: u64) -> Self {
        Self(id)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn from_str(src: &str) -> Result<ElementId, std::num::ParseIntError> {
        Ok(ElementId(u64::from_str_radix(src, 16)?))
    }

    pub fn to_chars(&self) -> [char; 16] {
        let hex_string = format!("{:016x}", self.0); // 格式化为16位十六进制字符串
        let mut char_array = ['0'; 16]; // 初始化一个字符数组
        for (i, c) in hex_string.chars().enumerate() {
            char_array[i] = c; // 填充字符数组
        }
        char_array
    }
}
