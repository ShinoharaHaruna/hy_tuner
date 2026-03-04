#[derive(Debug, Clone)]
pub enum Field {
    TestFileUrl,
    LatencyUrl,
    MinUp,
    MaxUp,
    MinDown,
    MaxDown,
    TargetAccuracy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Params,
    Logs,
}
