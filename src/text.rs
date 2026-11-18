pub(crate) fn lower(units: &[u16]) -> Vec<u16> {
    map_chars(units, char::to_lowercase)
}

/// Applies `Character.toLowerCase(char)` independently to each Java UTF-16
/// code unit, which is the behavior used by Linotte's case-insensitive search.
pub(crate) fn lower_simple(units: &[u16]) -> Vec<u16> {
    units
        .iter()
        .map(|unit| {
            let Some(character) = char::from_u32(u32::from(*unit)) else {
                return *unit;
            };
            character
                .to_lowercase()
                .next()
                .filter(|mapped| u32::from(*mapped) <= u32::from(u16::MAX))
                .map_or(*unit, |mapped| mapped as u16)
        })
        .collect()
}

fn map_chars(units: &[u16], map: fn(char) -> std::char::ToLowercase) -> Vec<u16> {
    let mut output = Vec::with_capacity(units.len());
    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(character) => {
                for mapped in map(character) {
                    let mut buffer = [0; 2];
                    output.extend_from_slice(mapped.encode_utf16(&mut buffer));
                }
            }
            Err(error) => output.push(error.unpaired_surrogate()),
        }
    }
    output
}

pub(crate) fn uppercase(units: &[u16]) -> Vec<u16> {
    let mut output = Vec::with_capacity(units.len());
    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(character) => {
                for mapped in character.to_uppercase() {
                    let mut buffer = [0; 2];
                    output.extend_from_slice(mapped.encode_utf16(&mut buffer));
                }
            }
            Err(error) => output.push(error.unpaired_surrogate()),
        }
    }
    output
}

pub(crate) fn capitalize(units: &[u16]) -> Vec<u16> {
    let mut marked = units.to_vec();
    marked.push(u16::from(b'*'));
    let mut parts = Vec::new();
    let mut start = 0;
    for (index, unit) in marked.iter().enumerate() {
        if matches!(*unit, 9 | 10 | 11 | 12 | 13 | 32) {
            parts.push(lower(&marked[start..index]));
            start = index + 1;
        }
    }
    parts.push(lower(&marked[start..]));

    let mut output = Vec::with_capacity(marked.len());
    let last = parts.len().saturating_sub(1);
    for (index, mut part) in parts.into_iter().enumerate() {
        if let Some(first) = part.first_mut() {
            if let Some(character) = char::from_u32(u32::from(*first)) {
                let mut uppercase = character.to_uppercase();
                if let Some(mapped) = uppercase.next()
                    && uppercase.next().is_none()
                    && u32::from(mapped) <= u32::from(u16::MAX)
                {
                    *first = mapped as u16;
                }
            }
        }
        output.extend(part);
        if index != last {
            output.push(u16::from(b' '));
        }
    }
    output.pop();
    output
}

pub(crate) fn reverse(units: &[u16]) -> Vec<u16> {
    let mut output = units.to_vec();
    output.reverse();
    let mut index = 0;
    while index + 1 < output.len() {
        if is_low(output[index]) && is_high(output[index + 1]) {
            output.swap(index, index + 1);
            index += 2;
        } else {
            index += 1;
        }
    }
    output
}

pub(crate) fn find(haystack: &[u16], needle: &[u16], start: usize) -> Option<usize> {
    let tail = haystack.get(start..)?;
    if needle.is_empty() {
        return Some(start);
    }
    tail.windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

pub(crate) fn replace(haystack: &[u16], needle: &[u16], replacement: &[u16]) -> Vec<u16> {
    if needle.is_empty() {
        let mut output =
            Vec::with_capacity(haystack.len() + (haystack.len() + 1) * replacement.len());
        output.extend(replacement);
        for unit in haystack {
            output.push(*unit);
            output.extend(replacement);
        }
        return output;
    }
    let mut output = Vec::with_capacity(haystack.len());
    let mut start = 0;
    while let Some(index) = find(haystack, needle, start) {
        output.extend(&haystack[start..index]);
        output.extend(replacement);
        start = index + needle.len();
    }
    output.extend(&haystack[start..]);
    output
}

fn is_high(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

fn is_low(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}
