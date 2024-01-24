use std::fmt;

// https://github.com/Holo-Host/holo-nixpkgs/blob/develop/profiles/logical/happ-releases.nix#L9C5-L9C5
pub const HHA_URL: &str = "https://holo-host.github.io/holo-hosting-app-rsm/releases/downloads/core-app/0_5_27/core-app.0_5_27-skip-proof.happ";
pub const SL_URL: &str = "https://holo-host.github.io/servicelogger-rsm/releases/downloads/0_4_21/servicelogger.0_4_21.happ";

pub enum Happ {
    HHA,
    SL,
}
// Converts enum to happ id, has to match one derived from /resources/test/config.yaml
impl fmt::Display for Happ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Happ::HHA => write!(f, "core-app:0_5_27"),
            Happ::SL => write!(f, "servicelogger"),
        }
    }
}
