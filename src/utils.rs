use crate::drawing::Coord;
use std::ops::RangeBounds;


pub fn chars_amount(string: &str) -> usize {
    string.chars().count()
}

pub fn truncate_with_delimiter(string: &str, max_length: Coord) -> String {
    let chars_amount = chars_amount(&string);
    if chars_amount > max_length as usize {
        let delimiter = "...";
        let leave_at_end = 5;
        let total_end_len = leave_at_end + delimiter.len();
        let start = max_length as usize - total_end_len;
        let end = chars_amount - leave_at_end;
        replace_range_with(string, start..end, delimiter)
    } else {
        string.clone().to_string()
    }
}

pub fn maybe_truncate(string: &str, max_length: usize) -> String {
    let mut result = String::new();
    let string = string.replace("\r", "^M").replace("\t", "    "); // assume tab_size=4
    let mut chars = string.chars().take(max_length);
    while let Some(c) = chars.next() {
        result.push(c);
    }
    result
}


// Does not validate the range
// May implement in the future: https://crates.io/crates/unicode-segmentation
pub fn replace_range_with<R>(string: &str, chars_range: R, replacement: &str) -> String
        where R: RangeBounds<usize> {
    use std::ops::Bound::*;
    let start = match chars_range.start_bound() {
        Unbounded   => 0,
        Included(n) => *n,
        Excluded(n) => *n + 1,
    };
    let chars_count = string.chars().count(); // TODO: improve
    let end = match chars_range.end_bound() {
        Unbounded   => chars_count,
        Included(n) => *n + 1,
        Excluded(n) => *n,
    };

    let mut chars = string.chars();
    let mut result = String::new();
    for _ in 0..start { result.push(chars.next().unwrap()); } // push first part
    result.push_str(replacement); // push the replacement
    let mut chars = chars.skip(end - start); // skip this part in the original
    while let Some(c) = chars.next() { result.push(c); } // push the rest
    result
}

// The display is guaranteed to be able to contain 2*gap (accomplished in settings)
pub fn siblings_shift_for(gap: usize, max: usize, index: usize,
                          len: usize, old_shift: Option<usize>) -> usize {
    let gap   = gap   as Coord;
    let max   = max   as Coord;
    let index = index as Coord;
    let len   = len   as Coord;

    if len <= max         { return 0; }
    if index < gap        { return 0; }
    if index >= len - gap { return (len - max) as usize; }

    if let Some(old_shift) = old_shift {
        let old_shift = old_shift as Coord;

        let shift = index - gap;
        if shift < old_shift { return shift as usize; }
        let shift = index + 1 - max + gap;
        if shift > old_shift { return shift as usize; }

        old_shift as usize
    } else { // no requirements => let at the top of the screen after the gap
        let mut shift = index - gap;
        let left_at_bottom = len - shift - max;
        if left_at_bottom < 0 { shift += left_at_bottom; }
        shift as usize
    }
}
