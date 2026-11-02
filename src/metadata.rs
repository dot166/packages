use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub time: i64,

    pub packages: HashMap<String, Package>,

    #[serde(default)]
    pub fs_verity_certs: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(default)]
    pub signatures: Vec<String>,

    #[serde(default)]
    pub source: Option<String>,

    #[serde(default)]
    pub original_package: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub icon_type: Option<String>,

    #[serde(default)]
    pub icon_sha256: Option<String>,

    #[serde(default)]
    pub icon_size: Option<u64>,

    #[serde(default)]
    pub is_top_level: Option<bool>,

    #[serde(default)]
    pub static_deps: Vec<String>,

    #[serde(default)]
    pub deps: Vec<String>,

    pub variants: HashMap<String, Variant>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub version_code: u64,

    #[serde(default)]
    pub version_name: Option<String>,

    #[serde(default)]
    pub label: Option<String>,

    #[serde(default)]
    pub channel: Option<String>,

    #[serde(default)]
    pub min_sdk: Option<u32>,

    #[serde(default)]
    pub apks: Vec<String>,

    #[serde(default)]
    pub apk_hashes: Vec<String>,

    #[serde(default)]
    pub apk_sizes: Vec<u64>,

    #[serde(default)]
    pub apk_gz_sizes: Vec<u64>,

    #[serde(default)]
    pub apk_br_sizes: Vec<u64>,

    #[serde(default)]
    pub apk_br_hashes: Vec<String>,

    #[serde(default)]
    pub abis: Vec<String>,

    #[serde(default)]
    pub has_v4_signatures: bool,

    #[serde(default)]
    pub v4_signature_sizes: Vec<u64>,

    #[serde(default)]
    pub v4_signature_hashes: Vec<String>,

    #[serde(default)]
    pub has_fs_verity_signatures: bool,

    #[serde(default)]
    pub fsv_signature_sizes: Vec<u64>,

    #[serde(default)]
    pub fsv_signature_hashes: Vec<String>,

    #[serde(default)]
    pub available_delta_versions: Vec<u64>,

    #[serde(default)]
    pub apk_delta_src_hashes: Vec<String>,

    #[serde(default)]
    pub apk_delta_sizes: Vec<u64>,

    #[serde(default)]
    pub apk_delta_hashes: Vec<String>,
}
