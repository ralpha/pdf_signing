use crate::Error;
use std::ops::RangeInclusive;

#[derive(Debug, Clone)]
pub struct ByteRange(pub Vec<usize>);

impl ByteRange {
    pub fn to_list(&self, fixed_width: usize) -> Result<String, Error> {
        let range = self.0.clone();
        let list_string = range
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<String>>()
            .join(" ");
        if fixed_width < list_string.len() {
            Err(Error::from(format!(
                "ByteRange `fixed_width` is to small. Current: `{}`, Expected at least: `{}`",
                fixed_width,
                list_string.len()
            )))
        } else {
            Ok(format!(
                "{}{}",
                list_string,
                " ".repeat(fixed_width - list_string.len())
            ))
        }
    }

    pub fn get_range(&self, range_pair_index: usize) -> RangeInclusive<usize> {
        let index = range_pair_index * 2;
        if index > self.0.len() {
            panic!("Index range out of bounds: TODO");
        }
        self.0[index]..=self.0[index] + self.0[index + 1] - 1
    }

    pub fn get_capacity_inclusive(&self) -> usize {
        let mut total = 0;
        let mut offset_value = None;
        for value in &self.0 {
            match offset_value {
                Some(_offset_value) => {
                    total += value;
                    offset_value = None;
                }
                None => {
                    offset_value = Some(value);
                }
            }
        }
        total
    }
}
