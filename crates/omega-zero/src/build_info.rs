#[derive(Clone, Debug)]
pub struct Build {
    /// The version it goes by: the one it was released as, or the name it took since.
    pub version: String,
    pub hash: &'static str,
    pub dirty: bool,
    pub date: &'static str,
}

impl Build {
    pub fn current() -> Build {
        Build {
            // a test build's own, `2.7.3-test.12`, as Update reads it
            version: crate::update::running(),
            hash: env!("OMEGA_GIT_HASH"),
            dirty: env!("OMEGA_GIT_DIRTY") == "1",
            date: env!("OMEGA_BUILD_DATE"),
        }
    }

    pub fn serial(&self) -> String {
        self.hash.to_uppercase()
    }

    /// The build as About's BUILD row shows it: the commit's short hash in capitals, and
    /// `-DIRTY` after it for a build made with changes not yet committed, as Git's own
    /// `describe --dirty` marks one.
    pub fn shown(&self) -> String {
        match self.dirty {
            true => format!("{}-DIRTY", self.serial()),
            false => self.serial(),
        }
    }
}
