//! Download the Text-Fabric files that `build` needs (only in copies built with `--features download`).

use dss_search::builder::{CORE_FILES, FEATURES, RAW_URL};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn download_features(cache: &Path, reference: &str, version: &str) -> Result<PathBuf, String> {
    let target = cache.join(version);
    std::fs::create_dir_all(&target).map_err(|e| format!("Could not create {}: {e}", target.display()))?;
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(30)).timeout_read(Duration::from_secs(120)).build();
    for name in CORE_FILES.iter().chain(FEATURES.iter()) {
        let path = target.join(format!("{name}.tf"));
        if path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            continue;
        }
        let url = RAW_URL.replace("{ref}", reference).replace("{version}", version).replace("{name}", name);
        println!("  downloading {name}.tf");
        let mut last_error = String::new();
        let mut done = false;
        for attempt in 0..3 {
            if attempt > 0 {
                std::thread::sleep(Duration::from_secs(2));
            }
            let result = agent.get(&url).call().map_err(|e| e.to_string()).and_then(|response| {
                let mut bytes = Vec::new();
                response.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                Ok(bytes)
            });
            match result {
                Ok(bytes) => {
                    // write to a temporary name first, so that an interrupted download is not mistaken for a complete file
                    let partial = path.with_extension("tf.part");
                    std::fs::write(&partial, bytes)
                        .and_then(|_| std::fs::rename(&partial, &path))
                        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
                    done = true;
                    break;
                }
                Err(e) => last_error = e,
            }
        }
        if !done {
            return Err(format!("Could not download {url}: {last_error}"));
        }
    }
    Ok(target)
}
