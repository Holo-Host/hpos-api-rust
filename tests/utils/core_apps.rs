use std::fmt;

// https://github.com/Holo-Host/holo-nixpkgs/blob/develop/profiles/logical/happ-releases.nix#L9C5-L9C5
pub const HHA_URL: &str = "https://holo-host.github.io/holo-hosting-app-rsm/releases/downloads/core-app/0_5_19/core-app.0_5_19-skip-proof.happ";
pub const SL_URL: &str = "https://holo-host.github.io/servicelogger-rsm/releases/downloads/0_4_19/servicelogger.0_4_19.happ";
pub const DD_URL: &str =
    "https://github.com/Holo-Host/dummy-dna/releases/download/0.6.10/test-skip-proof.happ";

pub enum Happ {
    HHA,
    SL,
    DummyDna,
}
// Converts enum to happ id, has to match one derived from /resources/test/config.yaml
impl fmt::Display for Happ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Happ::HHA => write!(f, "core-app:0_5_19"),
            Happ::SL => write!(f, "servicelogger"),
            Happ::DummyDna => write!(f, "dummy_dna"),
        }
    }
}
