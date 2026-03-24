use serde::{Deserialize, Serialize};
use std::{fs, process::Command};
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
struct Dict {
    id: String,
    locale: String,
    description: String,
    update: u128,
    filesize: u64,
    checksum: String,
    url: String,
    version: u128,
    formatversion: i32,
}

#[derive(Serialize, Deserialize)]
struct FinalDict {
    dicts: Vec<Dict>,
}

fn main() {
    let contents = Vec::from([
        "en_GB",
        "ja",
    ]);

    let jobs: Vec<String> = contents
        .into_iter()
        .map(String::from)
        .collect();

    let mut dicts = Vec::new();
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let time = since_the_epoch.as_millis();
    for job in jobs {
        let result: Result<Dict, String>;
        if job != "ja" {
            result = process_dict(job.clone(), time);
        } else {
            result = build_ja_dict_via_mozc(time);
        }
        if let Err(e) = result {
            panic!("{} failure: {}", job, e);
        } else if let Ok(list) = result {
            dicts.push(list);
        }
    }

    println!("Successfully processed {} dictionaries.", dicts.len());
    let dict_list = FinalDict {
        dicts,
    };
    fs::write("../../dicts.json", serde_json::to_string(&dict_list).unwrap()).unwrap();
    env::set_current_dir(&Path::new("../..")).unwrap();
    let status = Command::new("git")
        .arg("add")
        .arg(".")
        .status();

    if let Err(e) = status {
        panic!("Error adding changes: {}", e);
    }
    let status = Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg("[AUTO] Update dicts")
        .status();

    if let Err(e) = status {
        panic!("Error committing changes: {}", e);
    }
}

fn process_dict(
    job: String,
    time: u128,
) -> Result<Dict, String> {
    let dir_name = hun_loc(job.clone(), true);
    let dict_name = hun_loc(job.clone(), false);
    let api_url = format!(
        "https://raw.githubusercontent.com/LibreOffice/dictionaries/refs/heads/master/{}/{}.dic",
        dir_name.clone(),
        dict_name.clone()
    );

    let client = reqwest::blocking::Client::builder()
        .user_agent("gen-dicts/0.1.0 (https://github.com/dot166/packages)")
        .build()
        .unwrap();

    let resp = client
        .get(api_url)
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }

    let body = resp.text()
        .map_err(|e| format!("read body: {}", e))?;

    let _ = fs::write(format!("dicts/{}.dic", dict_name.clone()), &body);

    let api_url = format!(
        "https://raw.githubusercontent.com/LibreOffice/dictionaries/refs/heads/master/{}/{}.aff",
        dir_name.clone(),
        dict_name.clone()
    );

    let resp = client
        .get(api_url)
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }

    let body = resp.text()
        .map_err(|e| format!("read body: {}", e))?;

    let _ = fs::write(format!("dicts/{}.aff", dict_name.clone()), &body);

    let status = Command::new("git")
        .args([
            "diff",
            "--quiet",
            format!("dicts/{}.aff", dict_name.clone()).as_str()
        ])
        .status();

    let changes_aff = if let Ok(status) = status {
        if status.success() {
            false
        } else {
            true
        }
    } else {
        true
    };

    println!("{}.aff-CHANGES={}", dict_name, changes_aff);

    let status = Command::new("git")
        .args([
            "diff",
            "--quiet",
            format!("dicts/{}.dic", dict_name.clone()).as_str()
        ])
        .status();

    let changes_dic = if let Ok(status) = status {
        if status.success() {
            false
        } else {
            true
        }
    } else {
        true
    };

    println!("{}.dic-CHANGES={}", dict_name, changes_dic);

    if !changes_aff || !changes_dic {
        // no update needed, use previous record
        if let Ok(dict) = get_previous_json_for_dict(&job) {
            return Ok(dict);
        } else {
            return Err(format!("No dictionary found for {}", job));
        }
    }
    let status = Command::new("./main.py") // this is the already existing python code, do not convert this bit
        .args(&[
            &job,
            &((time / 1000) / 60).to_string(),
            &time.to_string(),
            &get_description(job.clone())
        ])
        .status()
        .map_err(|e| format!("Failed to run python script: {}", e))?;

    if !status.success() {
        return Err(format!(
            "python failed for {} {}",
            &job,
            format!("logs: {}", fs::read_to_string(format!("{}-worker.log", &job)).unwrap())
        ));
    }
    let data = fs::read(format!("../../LIME/main_{}.dict", &job.to_lowercase())).expect(format!("Unable to read file for {}", &job.to_lowercase()).as_str());
    let digest = md5::compute(data);
    let checksum = format!("{:x}", digest);
    let version = (time / 1000) / 60;
    let id = format!("main:{}", job.to_lowercase());
    let metadata = fs::metadata(format!("../../LIME/main_{}.dict", &job.to_lowercase())).unwrap();
    let filesize = metadata.len();
    let dict = Dict {
        id,
        locale: job.clone(),
        description: get_description(job.clone()),
        update: time,
        filesize,
        checksum,
        url: format!("https://dot166.github.io/packages/LIME/main_{}.dict", &job.to_lowercase()),
        version,
        formatversion: 86736212
    };
    Ok(dict)
}

fn build_ja_dict_via_mozc(time: u128) -> Result<Dict, String> {
    println!("building dictionary for ja");
    let status = Command::new("git")
        .args([
            "clone",
            "https://github.com/google/mozc.git"
        ])
        .status()
        .map_err(|e| format!("Failed to clone mozc: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Failed to clone mozc: {}",
            format!("logs: {}", fs::read_to_string("ja-worker.log").unwrap())
        ));
    }

    env::set_current_dir(&Path::new("mozc/src")).unwrap();
    let status = Command::new("../../UT-dicts.sh")
        .status()
        .map_err(|e| format!("Failed to add UT dicts: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Failed to add UT dicts: {}",
            format!("logs: {}", fs::read_to_string("ja-worker.log").unwrap())
        ));
    }

    env::set_current_dir(&Path::new("../..")).unwrap();
    let status = Command::new("git")
        .args([
            "diff",
            "--quiet",
            "dicts/mozcdic-ut.txt",
        ])
        .status();

    let changes = if let Ok(status) = status {
        if status.success() {
            false
        } else {
            true
        }
    } else {
        true
    };

    println!("mozc-CHANGES={}", changes);

    if !changes {
        fs::remove_dir_all("mozc").unwrap();
        // no update needed, use previous record
        if let Ok(dict) = get_previous_json_for_dict(&"ja".to_string()) {
            return Ok(dict);
        } else {
            return Err(format!("No dictionary found for ja"));
        }
    }
    env::set_current_dir(&Path::new("mozc/src")).unwrap();

    let status = Command::new("python3")
        .arg("build_tools/update_deps.py")
        .status()
        .map_err(|e| format!("Failed to update mozc deps: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Failed to update mozc deps: {}",
            format!("logs: {}", fs::read_to_string("ja-worker.log").unwrap())
        ));
    }

    let status = Command::new("bazelisk"
        ).args([
            "build",
            "//data_manager/oss:mozc_dataset_for_oss",
            "--config",
            "linux",
            "--config",
            "release_build"
        ])
        .status()
        .map_err(|e| format!("Failed to build mozc dictionary: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Failed to build mozc dictionary: {}",
            format!("logs: {}", fs::read_to_string("ja-worker.log").unwrap())
        ));
    }
    fs::copy("bazel-bin/data_manager/oss/mozc.data", "../../../../LIME/mozc.data").unwrap();
    env::set_current_dir(&Path::new("../..")).unwrap();
    fs::remove_dir_all("mozc").unwrap();
    println!("finished building dictionary for ja");
    let data = fs::read("../../LIME/mozc.data").expect("Unable to read file");
    let digest = md5::compute(data);
    let checksum = format!("{:x}", digest);
    let version = (time / 1000) / 60;
    let id = format!("mozc.data");
    let metadata = fs::metadata("../../LIME/mozc.data").unwrap();
    let filesize = metadata.len();
    let dict = Dict {
        id,
        locale: "ja".to_string(),
        description: get_description("ja".to_string()),
        update: time,
        filesize,
        checksum,
        url: "https://dot166.github.io/packages/LIME/mozc.data".to_string(),
        version,
        formatversion: 86736212
    };
    Ok(dict)
}

// should mirror function in wordlist.py, but this only contains the mapppings for what i need, instead of all of them, like the python one
fn hun_loc(loc: String, is_dir: bool) -> String {
    if loc.len() == 2 {
        if loc == "cs" {
            return "cs_CZ".to_string();
        } else if loc == "en" {
            if is_dir {
                return loc;
            } else {
                // i want to use en_GB, but AOSP maps en to en_US, so, match it
                println!("using en_US for locale en");
                return "en_US".to_string();
            }
        } else if loc == "de" {
            if is_dir {
                return loc;
            } else {
                return "de_DE_frami".to_string();
            }
        } else if loc == "es" {
            if is_dir {
                return loc;
            } else {
                return format!("{}_{}", loc, loc.to_uppercase());
            }
        } else if loc == "fr" {
            if is_dir {
                return format!("{}_{}", loc, loc.to_uppercase());
            } else {
                return loc;
            }
        } else {
            return format!("{}_{}", loc, loc.to_uppercase());
        }
    } else {
        let lang_vec: Vec<String> = loc.split("_").map(|f| f.to_string()).collect();
        let lang = lang_vec[0].clone();
        if lang == "en" && is_dir {
            return lang;
        } else {
            return loc;
        }
    }
}

fn get_previous_json_for_dict(job: &String) -> Result<Dict, String> {
    let json = fs::read_to_string("../../dicts.json")
        .map_err(|e| format!("json read: {}", e)).unwrap();
    let dicts: FinalDict =
        serde_json::from_str(&json)
        .map_err(|e| format!("JSON parse error: {}", e)).unwrap();
    for dict in dicts.dicts {
        if &dict.locale == job {
            return Ok(dict);
        }
    }

    // this should never happen, but handle it anyway
    Err(format!("No dictionary found for {}", job))
}

fn get_description(job: String) -> String {
    if job == "en_GB" {
        return "English (UK)".to_string();
    } else if job == "ja" {
        return "日本語".to_string();
    } else {
        return job; // just return the code, as clearly, some unknown language slipped in here...
    }
}

