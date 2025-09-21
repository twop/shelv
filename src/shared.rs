// TODO Move this to a shared lib once we migrate to Cargo workspaces

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version(pub String);

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionResponse {
    pub min_version: Version,
    pub latest_version: Version,
}
