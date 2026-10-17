// `core::time`.
pub struct Duration {
    secs: u64,
    nanos: u32,
}

impl Duration {
    pub const fn new(secs: u64, nanos: u32) -> Duration {
        Duration { secs, nanos }
    }
    pub const fn from_secs(secs: u64) -> Duration {
        Duration { secs, nanos: 0 }
    }
    pub const fn from_millis(millis: u64) -> Duration {
        Duration {
            secs: millis / 1000,
            nanos: ((millis % 1000) as u32) * 1000000,
        }
    }
    pub const fn as_secs(&self) -> u64 {
        self.secs
    }
    pub const fn subsec_nanos(&self) -> u32 {
        self.nanos
    }
}
