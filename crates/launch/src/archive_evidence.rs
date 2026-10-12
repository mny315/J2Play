use diagnostics::EmuError;
use std::collections::BTreeMap;
use std::path::Path;

/// Bounded static evidence collected from a suite archive without executing it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchiveEvidence {
    archive_name: Option<String>,
    external_classes: Vec<String>,
    descriptorless_suite_property_defaults: BTreeMap<String, String>,
}

impl ArchiveEvidence {
    /// Inspects class references, recognized descriptorless deployment
    /// contracts, and the archive's leaf filename.
    ///
    /// # Errors
    /// Returns a controlled archive or class-parser diagnostic.
    pub fn inspect_path(path: &Path) -> Result<Self, EmuError> {
        let static_evidence = compatibility::static_archive_evidence_from_jar_path(path)?;
        Ok(Self {
            archive_name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            external_classes: static_evidence.external_classes,
            descriptorless_suite_property_defaults: static_evidence
                .descriptorless_suite_property_defaults,
        })
    }

    /// Inspects archive bytes already opened by a frontend. Only the optional
    /// leaf filename is retained as weak selection evidence; a source URI or
    /// host path is neither required nor stored.
    ///
    /// # Errors
    /// Returns a controlled archive or class-parser diagnostic.
    pub fn inspect_bytes(archive_leaf_name: Option<&str>, bytes: &[u8]) -> Result<Self, EmuError> {
        let static_evidence = compatibility::static_archive_evidence_from_jar_bytes(bytes)?;
        Ok(Self {
            archive_name: archive_leaf_name
                .and_then(|name| name.rsplit(['/', '\\']).next())
                .filter(|name| !name.is_empty() && *name != "." && *name != "..")
                .map(str::to_owned),
            external_classes: static_evidence.external_classes,
            descriptorless_suite_property_defaults: static_evidence
                .descriptorless_suite_property_defaults,
        })
    }

    /// Creates evidence for project-owned tests or frontends which already
    /// performed bounded static archive inspection.
    #[must_use]
    pub fn new(
        archive_name: Option<String>,
        external_classes: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut external_classes = external_classes.into_iter().collect::<Vec<_>>();
        external_classes.sort();
        external_classes.dedup();
        Self {
            archive_name,
            external_classes,
            descriptorless_suite_property_defaults: BTreeMap::new(),
        }
    }

    /// Leaf archive name used only as a weak distribution hint.
    #[must_use]
    pub fn archive_name(&self) -> Option<&str> {
        self.archive_name.as_deref()
    }

    /// Sorted external class references found in application class files.
    #[must_use]
    pub fn external_classes(&self) -> &[String] {
        &self.external_classes
    }

    /// Adds neutral values for a recognized legacy deployment contract when
    /// the launch has no external JAD. Existing manifest values always win.
    /// Unknown properties retain normal MIDP `null` semantics.
    ///
    /// Returns the canonical names and values which were actually inserted.
    pub fn apply_descriptorless_suite_property_defaults(
        &self,
        properties: &mut BTreeMap<String, String>,
    ) -> Vec<(String, String)> {
        let mut applied = Vec::new();
        for (name, value) in &self.descriptorless_suite_property_defaults {
            let folded = name.to_ascii_lowercase();
            if properties.contains_key(&folded) {
                continue;
            }
            properties.insert(folded, value.clone());
            applied.push((name.clone(), value.clone()));
        }
        applied
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/launch/archive_evidence/mod.rs"]
mod tests;
