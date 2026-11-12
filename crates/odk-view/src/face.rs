//! The font handed to Rune before the window opens.
//!
//! Rune loads a family from bytes in `AppSpec::fonts`, and only then. It resolves fontconfig's
//! `monospace` slot on its own and has no public call that loads a family while a window is
//! open (`docs/checks.md`). The bytes live for the process.

/// One family, as bytes, for both of Rune's font slots.
#[derive(Clone, Copy)]
pub struct Face {
    fonts: &'static [&'static [u8]],
    families: (&'static str, &'static str),
}

impl Face {
    /// Rune's bundled Inter. An installed family is not used.
    pub const fn bundled() -> Self {
        Self {
            fonts: &[],
            families: ("Inter", "Inter"),
        }
    }

    /// Keep each of `files` for the process and name both slots `family`: the regular face and,
    /// when there is one, the bold.
    pub(crate) fn leak_files(files: Vec<Vec<u8>>, family: String) -> Self {
        let files: Vec<&'static [u8]> = files
            .into_iter()
            .map(|bytes| &*Box::leak(bytes.into_boxed_slice()))
            .collect();
        let fonts: &'static [&'static [u8]] = Box::leak(files.into_boxed_slice());
        let family: &'static str = Box::leak(family.into_boxed_str());
        Self {
            fonts,
            families: (family, family),
        }
    }

    /// How many font files the face is made of: the regular one, and the bold one when there is
    /// one. The bundled face has none of its own.
    pub fn faces(self) -> usize {
        self.fonts.len()
    }

    /// The family both slots are named.
    pub fn family(self) -> &'static str {
        self.families.0
    }

    /// Keep `bytes` for the process and name both slots `family`.
    ///
    /// Call this once for a given file. A later call leaks another copy.
    pub fn leak(bytes: Vec<u8>, family: String) -> Self {
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let fonts: &'static [&'static [u8]] = Box::leak(Box::new([bytes]));
        let family: &'static str = Box::leak(family.into_boxed_str());
        Self {
            fonts,
            families: (family, family),
        }
    }

    pub(crate) fn fonts(self) -> &'static [&'static [u8]] {
        self.fonts
    }

    pub(crate) fn families(self) -> (&'static str, &'static str) {
        self.families
    }
}
