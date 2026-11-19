use crate::extension::ExtensionId;

/// A package-local asset identifier. It is deliberately not an arbitrary path.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId {
    pub extension: ExtensionId,
    pub name: String,
}

impl ResourceId {
    pub fn new(extension: ExtensionId, name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        if name.is_empty()
            || name.starts_with('/')
            || name
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || name.contains('\\')
        {
            return Err("resource name must be a package-relative path".into());
        }
        Ok(Self { extension, name })
    }
}
