use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, process::Command};
use std::collections::HashMap;
use chrono::{NaiveDate, NaiveTime};
use regex::Regex;
use std::error::Error;

#[derive(Serialize, Deserialize, Clone)]
struct Dictionary {
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

struct DictionaryPair {
    time: i64,
    url: String,
}

#[derive(Debug, Clone)]
struct DictEntry {
    url: String,
    date: NaiveDate,
}

#[derive(Debug)]
struct LanguageComparison {
    normal: Option<DictEntry>,
    experimental: Option<DictEntry>,
}

fn dict_new() -> Result<HashMap<String, DictionaryPair>, Box<dyn Error>> {
    let raw_url = "https://codeberg.org/Helium314/aosp-dictionaries/raw/branch/main/README.md";
    let client = reqwest::blocking::Client::builder()
        .user_agent("gen-dicts/0.1.0 (https://github.com/dot166/packages)")
        .build()?;
    let body = client.get(raw_url).send()?.text()?;
    let row_regex = Regex::new(
        r"(?x)
        ^\|\s*([^|]+?)\s*
        \|\s*\[([^]]+)]\(([^)]+)\)\s*
        \|\s*(yes|no)\s*
        \|\s*[^|]+\s*
        \|\s*[^|]+\s*
        \|\s*(\d{4}-\d{2}-\d{2})\s*\|
    ",
    )?;
    let mut language_groups: HashMap<String, LanguageComparison> = HashMap::new();
    for line in body.lines() {
        if let Some(caps) = row_regex.captures(line) {
            let language = caps[1].trim().to_string();
            let dict_type = caps[2].trim().to_string();
            let url = caps[3].trim().to_string();
            let is_experimental = &caps[4] == "yes";
            let date = match NaiveDate::parse_from_str(&caps[5], "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue, // Skip rows with invalid dates
            };
            if dict_type != "main" {
                continue;
            }
            let entry = DictEntry {
                url,
                date,
            };
            let comp = language_groups.entry(language).or_insert(LanguageComparison {
                normal: None,
                experimental: None,
            });
            if is_experimental {
                comp.experimental = Some(entry);
            } else {
                comp.normal = Some(entry);
            }
        }
    }
    let mut languages: Vec<&String> = language_groups.keys().collect();
    languages.sort();
    let mut results: HashMap<String, DictionaryPair> = HashMap::new();
    let time_zero = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    for lang in languages {
        if let Some(comp) = language_groups.get(lang) {
            match (&comp.normal, &comp.experimental) {
                (Some(normal), Some(experimental)) => {
                    if experimental.date > normal.date {
                        results.insert(format_locale(&experimental.url), DictionaryPair{ url: experimental.url.clone(), time: experimental.date.and_time(time_zero).and_utc().timestamp_millis() });
                    } else {
                        results.insert(format_locale(&normal.url), DictionaryPair{ url: normal.url.clone(), time: normal.date.and_time(time_zero).and_utc().timestamp_millis() });
                    }
                }
                (Some(normal), None) => {
                    results.insert(format_locale(&normal.url), DictionaryPair{ url: normal.url.clone(), time: normal.date.and_time(time_zero).and_utc().timestamp_millis() });
                }
                (None, Some(experimental)) => {
                    results.insert(format_locale(&experimental.url), DictionaryPair{ url: experimental.url.clone(), time: experimental.date.and_time(time_zero).and_utc().timestamp_millis() });
                }
                (None, None) => {
                    eprintln!("No dictionary available for {}", lang);
                }
            }
        }
    }

    Ok(results)
}

fn main() {
    let contents = Vec::from([
        // stuff I use
        "en_gb",
        "ja",
        // bundled AOSP dicts
        "en",
        "de",
        "es",
        "fr",
        "it",
        "pt_br",
        "ru",
        // potentially ones I want to support in the future go below this comment
    ]);

    let jobs: Vec<String> = contents
        .into_iter()
        .map(String::from)
        .collect();

    let mut dicts = Vec::new();

    for job in jobs {
        let result: Result<Dictionary, String>;
        if job != "ja" {
            result = process_dict(job.clone());
        } else {
            result = build_ja_dict_via_mozc();
        }
        if let Err(e) = result {
            panic!("{} failure: {}", job, e);
        } else if let Ok(list) = result {
            dicts.push(list);
        }
    }

    println!("Successfully processed {} dictionaries.", dicts.len());
    fs::write("../../dicts.json", serde_json::to_string(&dicts).unwrap()).unwrap();
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
) -> Result<Dictionary, String> {
    let dict_map = dict_new().unwrap();
    let dict = dict_map.get(&get_loc(&job)).unwrap();
    let api_url = dict.url.clone();
    let time = dict.time.clone();

    let client = reqwest::blocking::Client::builder()
        .user_agent("gen-dicts/0.1.0 (https://github.com/dot166/packages)")
        .build()
        .unwrap();

    let resp = client
        .get(api_url)
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Codeberg returned {}", resp.status()));
    }

    let body = resp.text()
        .map_err(|e| format!("read body: {}", e))?;

    let _ = fs::write(format!("dicts/main_{}.dict", job.to_lowercase()), &body);

    let status = Command::new("git")
        .args([
            "diff",
            "--quiet",
            format!("dicts/main_{}.dict", job.to_lowercase()).as_str()
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

    println!("{}-CHANGES={}", job, changes);

    if !changes {
        // no update needed, use previous record
        return if let Ok(dict) = get_previous_json_for_dict(&job) {
            Ok(dict)
        } else {
            Err(format!("No dictionary found for {}", job))
        }
    }
    fs::copy(format!("dicts/main_{}.dict", job.to_lowercase()), format!("../../LIME/main_{}.dict", &job.to_lowercase())).unwrap();
    let data = fs::read(format!("../../LIME/main_{}.dict", &job.to_lowercase())).expect(format!("Unable to read file for {}", &job.to_lowercase()).as_str());
    let digest = md5::compute(data);
    let checksum = format!("{:x}", digest);
    let version = (time / 1000) / 60;
    let id = format!("main:{}", job.to_lowercase());
    let metadata = fs::metadata(format!("../../LIME/main_{}.dict", &job.to_lowercase())).unwrap();
    let filesize = metadata.len();
    let dict = Dictionary {
        id,
        locale: job.clone(),
        description: get_description(job.clone()),
        update: time as u128,
        filesize,
        checksum,
        url: format!("https://dot166.github.io/packages/LIME/main_{}.dict", &job.to_lowercase()),
        version: version as u128,
        formatversion: 86736212
    };
    Ok(dict)
}

fn build_ja_dict_via_mozc() -> Result<Dictionary, String> {
    println!("building dictionary for ja");
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let time = since_the_epoch.as_millis();
    let status = Command::new("git")
        .args([
            "clone",
            "https://github.com/google/mozc.git"
        ])
        .status()
        .map_err(|e| format!("Failed to clone mozc: {}", e))?;

    if !status.success() {
        return Err("Failed to clone mozc".to_string());
    }

    env::set_current_dir(&Path::new("mozc/src")).unwrap();
    let status = Command::new("../../UT-dicts.sh")
        .status()
        .map_err(|e| format!("Failed to add UT dicts: {}", e))?;

    if !status.success() {
        return Err("Failed to add UT dicts".to_string());
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
        return if let Ok(dict) = get_previous_json_for_dict(&"ja".to_string()) {
            Ok(dict)
        } else {
            Err("No dictionary found for ja".to_string())
        }
    }
    env::set_current_dir(&Path::new("mozc/src")).unwrap();

    let status = Command::new("python3")
        .arg("build_tools/update_deps.py")
        .status()
        .map_err(|e| format!("Failed to update mozc deps: {}", e))?;

    if !status.success() {
        return Err("Failed to update mozc deps".to_string());
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
        return Err("Failed to build mozc dictionary".to_string());
    }
    let status = Command::new("bash"
    ).args([
        "-c",
        "chmod 777 ../../../../LIME/mozc.data && rm -rf ../../../../LIME/mozc.data",
    ])
        .status()
        .map_err(|e| format!("Failed to build mozc dictionary: {}", e))?;

    if !status.success() {
        return Err("Failed to remove old mozc dictionary".to_string());
    }
    fs::copy("bazel-bin/data_manager/oss/mozc.data", "../../../../LIME/mozc.data").unwrap();
    env::set_current_dir(&Path::new("../..")).unwrap();
    fs::remove_dir_all("mozc").unwrap();
    println!("finished building dictionary for ja");
    let data = fs::read("../../LIME/mozc.data").expect("Unable to read file");
    let digest = md5::compute(data);
    let checksum = format!("{:x}", digest);
    let version = (time / 1000) / 60;
    let id = "main:ja".to_string();
    let metadata = fs::metadata("../../LIME/mozc.data").unwrap();
    let filesize = metadata.len();
    let dict = Dictionary {
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

fn get_previous_json_for_dict(job: &String) -> Result<Dictionary, String> {
    let json = fs::read_to_string("../../dicts.json")
        .map_err(|e| format!("json read: {}", e))?;
    let dicts: Vec<Dictionary> =
        serde_json::from_str(&json)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    for dict in dicts {
        if &dict.locale == job {
            return Ok(dict);
        }
    }

    // this should never happen, but handle it anyway
    Err(format!("No dictionary found for {}", job))
}

fn get_description(job: String) -> String {
    let mut log = String::new();
    let output = Command::new("java")
        //.current_dir(&cwd)
        .arg("-jar")
        .arg("kt/app.jar")
        .arg(job)
        .output()
        .expect("Error getting locale");
    log.push_str(match str::from_utf8(&output.stdout) {
        Ok(val) => val.trim(),
        Err(_) => panic!("got non UTF-8 data from java"),
    });
    log
}

fn format_locale(url: &String) -> String {
    let regex = Regex::new("main_.*\\.").unwrap();
    regex.captures(&*url).unwrap()[0].trim().to_string().replace("main_", "").replace(".", "")
}

fn get_loc(loc: &String) -> String {
    if loc.len() == 2 {
        if loc == "en" {
            // I want to use en_gb, but AOSP maps en to en_us, so, match it
            println!("using en_us for locale en");
            "en_us".to_string()
        } else {
            loc.to_lowercase()
            //format!("{}_{}", loc.to_lowercase(), loc.to_lowercase())
        }
    } else {
        loc.to_lowercase()
    }
}
