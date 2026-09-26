//! Manifest and JAD properties, continuation rules, and `MIDlet` declarations.

use super::bounded_io::read_regular_file_bounded;
use super::{JadInfo, MAX_JAD_BYTES, ManifestSection, MidletInfo};
use diagnostics::{Category, EmuError};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug)]
pub(super) struct ParsedManifest {
    pub(super) main: BTreeMap<String, String>,
    pub(super) sections: Vec<ManifestSection>,
}

#[derive(Default)]
struct PropertySectionBuilder {
    // Continuations address the current field by index so a long property name
    // is not compared again for every short continuation line.
    // A UTF-8 character can straddle a physical line boundary. Decode values
    // only after all continuation bytes have been appended.
    values: Vec<(String, Vec<u8>)>,
    folded_keys: BTreeMap<String, usize>,
}

/// Reads a bounded Java Application Descriptor.
///
/// # Errors
///
/// Returns a JAR-category diagnostic for I/O, size, encoding, or syntax errors.
pub fn parse_jad_path(path: impl AsRef<Path>) -> Result<JadInfo, EmuError> {
    let path = path.as_ref();
    let bytes = read_regular_file_bounded(
        path,
        MAX_JAD_BYTES,
        "jad-open",
        "jad-read",
        "jad-too-large",
        "JAD",
    )?;
    parse_jad(&bytes)
}

/// Parses JAD bytes with manifest-style continuation lines.
///
/// # Errors
///
/// Returns an error for oversized, malformed, duplicate, or invalid `MIDlet` fields.
pub fn parse_jad(bytes: &[u8]) -> Result<JadInfo, EmuError> {
    if bytes.len() > usize::try_from(MAX_JAD_BYTES).unwrap_or(usize::MAX) {
        return Err(EmuError::new(
            Category::Jar,
            "jad-too-large",
            "JAD exceeds 1 MiB",
        ));
    }
    let sections = parse_property_sections(bytes, "jad")?;
    if sections.len() != 1 {
        return Err(EmuError::new(
            Category::Jar,
            "jad-section",
            "JAD cannot contain individual manifest sections",
        ));
    }
    let properties = sections.into_iter().next().unwrap_or_default();
    let midlets = parse_midlets(&properties)?;
    Ok(JadInfo {
        properties,
        midlets,
    })
}

pub(super) fn parse_manifest(bytes: &[u8]) -> Result<ParsedManifest, EmuError> {
    let mut iter = parse_property_sections(bytes, "manifest")?.into_iter();
    let main = iter.next().unwrap_or_default();
    let mut named = Vec::new();
    for mut attributes in iter {
        let Some(name_key) = attributes
            .keys()
            .find(|key| key.eq_ignore_ascii_case("Name"))
            .cloned()
        else {
            // A sizeable body of deployed MIDlets has installer annotations
            // after a blank line without the mandatory `Name` attribute.
            // They are not part of the launch metadata, so match handset JAR
            // loaders and ignore only that malformed individual section.
            continue;
        };
        let name = attributes.remove(&name_key).unwrap_or_default();
        named.push(ManifestSection { name, attributes });
    }
    Ok(ParsedManifest {
        main,
        sections: named,
    })
}

fn parse_property_sections(
    bytes: &[u8],
    document: &'static str,
) -> Result<Vec<BTreeMap<String, String>>, EmuError> {
    let normalized_bytes = if document == "manifest" {
        normalize_spurious_manifest_lf(bytes)
    } else {
        Cow::Borrowed(bytes)
    };
    let mut sections = Vec::new();
    let mut section = PropertySectionBuilder::default();
    let mut current: Option<usize> = None;
    for line in property_lines(&normalized_bytes) {
        if line.is_empty() {
            current = None;
            if !section.values.is_empty() {
                sections.push(std::mem::take(&mut section).finish(document)?);
            }
            continue;
        }
        let recovered;
        let line = if let Some(continuation) = line.strip_prefix(b" ") {
            if let Some(index) = current {
                section.append(index, continuation)?;
                continue;
            }
            recovered = decode_property_text(continuation, document)?;
            let recovered = recovered.trim_start().as_bytes();
            if document == "manifest" && recovered.is_empty() {
                continue;
            }
            if document == "manifest" && recovered.contains(&b':') {
                recovered
            } else {
                return Err(EmuError::new(
                    Category::Jar,
                    "manifest-continuation",
                    "continuation without a preceding field",
                ));
            }
        } else {
            line
        };
        let Some(colon) = line.iter().position(|byte| *byte == b':') else {
            if document == "manifest"
                && let Some(index) = current
            {
                section.append(
                    index,
                    decode_property_text(line, document)?.trim().as_bytes(),
                )?;
                continue;
            }
            return Err(EmuError::new(
                Category::Jar,
                "manifest-field",
                format!("invalid manifest line: {}", String::from_utf8_lossy(line)),
            ));
        };
        let key = &line[..colon];
        let value = &line[colon + 1..];
        let value = value.strip_prefix(b" ").unwrap_or(value);
        if key.is_empty() {
            return Err(EmuError::new(
                Category::Jar,
                "manifest-field",
                "empty manifest field",
            ));
        }
        current = Some(section.insert(key, value, document)?);
    }
    if !section.values.is_empty() {
        sections.push(section.finish(document)?);
    }
    Ok(sections)
}

fn property_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes.split(|byte| *byte == b'\n').flat_map(|line| {
        // Retain acceptance of repeated CR before LF while recognizing bare
        // CR separators and the empty lines between manifest sections.
        let end = line
            .iter()
            .rposition(|byte| *byte != b'\r')
            .map_or(0, |index| index + 1);
        line[..end].split(|byte| *byte == b'\r')
    })
}

fn normalize_spurious_manifest_lf(bytes: &[u8]) -> Cow<'_, [u8]> {
    const MIXED_BLANK_LINE: &[u8] = b"\r\n\n";
    if !bytes
        .windows(MIXED_BLANK_LINE.len())
        .any(|part| part == MIXED_BLANK_LINE)
    {
        return Cow::Borrowed(bytes);
    }
    let mut output = Vec::with_capacity(bytes.len());
    let mut remaining = bytes;
    while let Some(offset) = remaining
        .windows(MIXED_BLANK_LINE.len())
        .position(|part| part == MIXED_BLANK_LINE)
    {
        output.extend_from_slice(&remaining[..offset]);
        let after = &remaining[offset + MIXED_BLANK_LINE.len()..];
        let starts_named_section = after
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"Name:"));
        output.extend_from_slice(if starts_named_section {
            MIXED_BLANK_LINE
        } else {
            b"\r\n"
        });
        remaining = after;
    }
    output.extend_from_slice(remaining);
    Cow::Owned(output)
}

impl PropertySectionBuilder {
    fn append(&mut self, index: usize, continuation: &[u8]) -> Result<(), EmuError> {
        let (_, value) = self.values.get_mut(index).ok_or_else(|| {
            EmuError::new(
                Category::Jar,
                "manifest-continuation",
                "missing preceding field",
            )
        })?;
        value.extend_from_slice(continuation);
        Ok(())
    }

    fn finish(self, document: &'static str) -> Result<BTreeMap<String, String>, EmuError> {
        self.values
            .into_iter()
            .map(|(key, value)| {
                let value = String::from_utf8(value).or_else(|error| {
                    decode_property_text(error.as_bytes(), document).map(Cow::into_owned)
                })?;
                Ok((key, value))
            })
            .collect()
    }

    fn insert(
        &mut self,
        key: &[u8],
        value: &[u8],
        document: &'static str,
    ) -> Result<usize, EmuError> {
        let key = decode_property_text(key, document)?;
        let folded = key.to_ascii_lowercase();
        if let Some(&existing) = self.folded_keys.get(&folded) {
            if document != "manifest" {
                return Err(EmuError::new(
                    Category::Jar,
                    "manifest-field",
                    format!("duplicate manifest field: {key}"),
                ));
            }
            value.clone_into(&mut self.values[existing].1);
            Ok(existing)
        } else {
            let index = self.values.len();
            self.values.push((key.into_owned(), value.to_owned()));
            self.folded_keys.insert(folded, index);
            Ok(index)
        }
    }
}

fn decode_property_text<'a>(
    bytes: &'a [u8],
    document: &'static str,
) -> Result<Cow<'a, str>, EmuError> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(Cow::Borrowed(text)),
        Err(_error) if document == "manifest" => {
            // Some feature-phone toolchains emitted localized manifest values
            // in an 8-bit code page. Attribute syntax is ASCII, and launch
            // must not fail solely because a display label is malformed.
            Ok(String::from_utf8_lossy(bytes))
        }
        Err(error) => Err(EmuError::with_source(
            Category::Jar,
            "property-encoding",
            format!("{document} is not UTF-8"),
            error,
        )),
    }
}

pub(super) fn parse_midlets(
    manifest: &BTreeMap<String, String>,
) -> Result<Vec<MidletInfo>, EmuError> {
    let mut midlets = Vec::new();
    for (key, value) in manifest {
        let Some(prefix) = key.get(..7) else {
            continue;
        };
        if !prefix.eq_ignore_ascii_case("MIDlet-") {
            continue;
        }
        let suffix = &key[7..];
        let Ok(index) = suffix.parse::<u32>() else {
            continue;
        };
        if index == 0 {
            continue;
        }
        let mut parts = value.splitn(4, ',').map(str::trim);
        match [parts.next(), parts.next(), parts.next(), parts.next()] {
            [Some(name), Some(icon), Some(class_name), None]
                if !name.is_empty() && !class_name.is_empty() =>
            {
                midlets.push(MidletInfo {
                    index,
                    name: name.to_owned(),
                    icon: (!icon.is_empty()).then(|| icon.to_owned()),
                    class_name: class_name.to_owned(),
                });
            }
            _ => {
                return Err(EmuError::new(
                    Category::Jar,
                    "midlet-entry",
                    format!("invalid {key} entry"),
                ));
            }
        }
    }
    midlets.sort_by_key(|midlet| midlet.index);
    if midlets
        .windows(2)
        .any(|pair| pair[0].index == pair[1].index)
    {
        return Err(EmuError::new(
            Category::Jar,
            "midlet-entry",
            "duplicate MIDlet index",
        ));
    }
    Ok(midlets)
}

#[cfg(test)]
#[path = "../../../tests/unit/jar/properties.rs"]
mod tests;
