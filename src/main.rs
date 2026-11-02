mod metadata;
mod decompress;

use anyhow::{anyhow, Context, Result};
use metadata::Metadata;
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, write, File};
use std::io::{BufReader, Read, copy};
use std::path::{Path, PathBuf};

const METADATA_URL: &str =
    "https://apps.grapheneos.org/metadata.1.0.sjson";

const RAW_BASE: &str =
    "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages";

const PACKAGE_BASE: &str =
    "https://apps.grapheneos.org/packages";

const BLACKLIST: &[&str] = &[
    "app.grapheneos.apps",
    "app.attestation.auditor",
    "app.grapheneos.info",
];

fn download(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<()> {
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("Request failed: {url}"))?
        .error_for_status()?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut output = File::create(destination)?;

    copy(&mut response, &mut output)
        .with_context(|| format!("Failed to save {url}"))?;

    Ok(())
}

fn download_if_available(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<bool> {
    let response = client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()?;

    if !response.status().is_success() {
        return Ok(false);
    }

    download(client, url, destination)?;

    Ok(true)
}

fn preserve_timestamp(
    source: &Path,
    destination: &Path,
) -> Result<()> {
    let modified = fs::metadata(source)?.modified()?;

    let file = File::options()
        .write(true)
        .open(destination)?;

    file.set_times(
        fs::FileTimes::new()
            .set_modified(modified),
    )?;

    Ok(())
}

fn download_metadata(client: &Client) -> Result<Metadata> {
    println!("Fetching GrapheneOS metadata...");

    let path = Path::new("metadata.sjson");

    download(client, METADATA_URL, path)?;

    let contents = fs::read_to_string(path)?;
    let mut deserializer = serde_json::Deserializer::from_str(&contents);

    let metadata = Metadata::deserialize(&mut deserializer)
        .context("Failed to parse GrapheneOS metadata")?;

    println!(
        "Loaded {} packages (metadata timestamp: {})",
        metadata.packages.len(),
        metadata.time
    );

    Ok(metadata)
}


fn process_package(
    client: &Client,
    name: &str,
    package: &metadata::Package,
) -> Result<()> {
    println!("Package: {name}");

    let package_dir = PathBuf::from("apps/packages").join(name);

    fs::create_dir_all(&package_dir)?;

    let common_props = package_dir.join("common-props.toml");

    if !common_props.exists() {
        let url = format!("{RAW_BASE}/{name}/common-props.toml");

        download_if_available(
            client,
            &url,
            &common_props,
        )?;
    }

    let icon = package_dir.join("icon.webp");

    if !icon.exists() {
        let url = format!("{RAW_BASE}/{name}/icon.webp");

        download_if_available(
            client,
            &url,
            &icon,
        )?;
    }

    for (version, variant) in &package.variants {
        if variant.channel.as_deref() != Some("stable") {
            println!("{name} v{version} is not yet stable");
            continue;
        }

        let version_dir = package_dir.join(version);

        fs::create_dir_all(&version_dir)?;

        let raw_url = format!("{RAW_BASE}/{name}/{version}");

        for filename in ["channel.toml", "props.toml"] {
            let destination = version_dir.join(filename);

            if !destination.exists() {
                let url = format!("{raw_url}/{filename}");

                download_if_available(
                    client,
                    &url,
                    &destination,
                )?;
            }
        }

        let package_url = format!(
            "{PACKAGE_BASE}/{name}/{version}"
        );
        let num_deltas = variant.available_delta_versions.len();
        let mut deltas_offset = 0;

        for (((((apk_name, apk_hash), br_hash), apk_size), br_size), gz_size) in variant.apks.iter().zip(&variant.apk_hashes).zip(&variant.apk_br_hashes).zip(&variant.apk_sizes).zip(&variant.apk_br_sizes).zip(&variant.apk_gz_sizes) {
            let br = version_dir.join(format!("{apk_name}.br"));
            let apk = version_dir.join(apk_name);
            let gz = version_dir.join(format!("{apk_name}.gz"));

            if !br.exists() {
                let url = format!("{package_url}/{apk_name}.br");

                download(
                    client,
                    &url,
                    &br,
                )?;
            }

            check_hash(&br, br_hash)?;
            check_size(&br, br_size)?;

            if !apk.exists() {
                decompress::decompress_brotli(
                    &br,
                    &apk,
                )?;
            }
            check_hash(&apk, apk_hash)?;
            check_size(&apk, apk_size)?;

            preserve_timestamp(&apk, &br)?;
            if !gz.exists() {
                let url = format!("{package_url}/{apk_name}.gz");

                download(
                    client,
                    &url,
                    &gz,
                )?;
            }
            check_size(&gz, gz_size)?;
            preserve_timestamp(&apk, &gz)?;

            if variant.has_v4_signatures {
                let idsig = version_dir.join(
                    format!("{apk_name}.idsig")
                );

                if !idsig.exists() {
                    let url = format!(
                        "{package_url}/{apk_name}.idsig"
                    );

                    download(
                        client,
                        &url,
                        &idsig,
                    )?;
                }
            }
            if num_deltas != 0 {
                let delta_sizes = array_range(&variant.apk_delta_sizes, Some(deltas_offset), Some(deltas_offset + num_deltas))?;
                for delta_version in &variant.available_delta_versions {
                    let i = variant.available_delta_versions.iter().position(|r| r == delta_version).unwrap();
                    let delta_hash = &variant.apk_delta_hashes[deltas_offset + i];
                    let delta = version_dir.join(
                        format!("{apk_name}.{delta_version}.ina")
                    );

                    if !delta.exists() {
                        let url = format!(
                            "{package_url}/{apk_name}.{delta_version}.ina"
                        );

                        download(
                            client,
                            &url,
                            &delta,
                        )?;
                    }
                    check_hash(&delta, delta_hash)?;
                    check_size(&delta, delta_sizes[i])?;

                    let delta_sha_file = version_dir.join(
                        format!("{apk_name}.{delta_version}.ina.sha256")
                    );

                    if !delta_sha_file.exists() {
                        write(delta_sha_file, delta_hash)?;
                    }
                }
            }
            deltas_offset += num_deltas;
        }
    }

    Ok(())
}

fn array_range<T>(
    ja: &Vec<T>,
    from: Option<usize>,
    to: Option<usize>,
) -> Result<Vec<&T>> {
    let from = from.unwrap_or(0);
    let to = to.unwrap_or(ja.len());
    let len = to.checked_sub(from).ok_or(anyhow!("`to` must be greater than or equal to `from`"))?;

    if len == 0 {
        return Ok(Vec::new());
    }

    let mut res = Vec::with_capacity(len);
    for i in 0..len {
        let value = ja
            .get(from + i)
            .ok_or_else(|| anyhow!(format!("index {} out of bounds", from + i)))?;
        res.push(value);
    }
    Ok(res)
}

fn check_size(file: &PathBuf, size_from_meta: &u64) -> Result<()> {
    let size = &fs::metadata(file)?.len();
    if size == size_from_meta {
        Ok(())
    } else {
        Err(anyhow!(format!("size mismatch, {} vs {}", size, size_from_meta)))
    }
}

fn check_hash(file: &PathBuf, hash_from_meta: &String) -> Result<()> {
    let mut file = BufReader::new(File::open(file)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0; 4096];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hex::encode(hasher.finalize());
    if hash.eq_ignore_ascii_case(hash_from_meta) {
        Ok(())
    } else {
        Err(anyhow!(format!("sha256 hash mismatch, {} vs {}", hash, hash_from_meta)))
    }
}

fn run() -> Result<()> {
    let client = Client::builder()
        .user_agent("graphene-package-mirror/1.0 (https://github.com/dot166/packages)")
        .build()?;

    let metadata = download_metadata(&client)?;

    for (name, package) in &metadata.packages {
        if BLACKLIST.contains(&name.as_str()) {
            println!("Skipping blacklisted package: {name}");
            continue;
        }

        process_package(
            &client,
            name,
            package,
        )
            .with_context(|| format!("Failed processing package {name}"))?;
    }

    println!("\nAll packages processed successfully.");

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
