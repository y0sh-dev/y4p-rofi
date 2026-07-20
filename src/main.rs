
// src/main.rs

use rayon::prelude::*;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const CACHE_DIR: &str = "/tmp/y1-clip-gui-cache";
const BIN: &str = "y4-clipboard";
const LIST_DEPTH: &str = "0-120";

fn main() {
    let _ = fs::create_dir_all(CACHE_DIR);

    // Fetch history from backend using Stable ID system
    let output = Command::new(BIN)
        .args(["list", LIST_DEPTH, "--raw", "--id"])
        .output();

    let Ok(out) = output else { return };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // Parallel processing for metadata and thumbnail extraction
    let rofi_items: Vec<String> = lines
        .into_par_iter()
        .filter_map(|line| {
            let start = line.find('[')? + 1;
            let end = line.find(']')?;
            let db_id = line[start..end].trim();
            
            if db_id.is_empty() { return None; }

            let icon_path = if line.contains("[IMG]") {
                get_image_icon(db_id)
            } else if line.contains("[FIL]") {
                "folder".to_string()
            } else {
                "text-x-generic".to_string()
            };

            Some(format!("{}\0icon\x1f{}", line, icon_path))
        })
        .collect();

    if !rofi_items.is_empty() {
        invoke_rofi(rofi_items);
    }
}

/// Resolves image path and performs on-demand extraction if not cached
fn get_image_icon(db_id: &str) -> String {
    let extensions = ["png", "jpg", "webp", "gif"];
    for ext in &extensions {
        let path = format!("{}/{}.{}", CACHE_DIR, db_id, ext);
        if Path::new(&path).exists() { return path; }
    }

    let output = Command::new(BIN)
        .args(["show", db_id, "--raw", "--id"])
        .output();

    if let Ok(out) = output {
        let data = out.stdout;
        if data.len() >= 4 {
            let ext = match &data[0..4] {
                [0x89, 0x50, 0x4E, 0x47] => "png",
                [0xFF, 0xD8, 0xFF, _]    => "jpg",
                [0x47, 0x49, 0x46, 0x38] => "gif",
                b"RIFF" if data.len() >= 12 && &data[8..12] == b"WEBP" => "webp",
                _ => "png",
            };

            let final_path = format!("{}/{}.{}", CACHE_DIR, db_id, ext);
            let _ = fs::write(&final_path, data);
            return final_path;
        }
    }
    "image-missing".to_string()
}

/// Renders the selection menu and synchronizes the chosen ID via IPC
fn invoke_rofi(items: Vec<String>) {
    let mut child = Command::new("rofi")
        .args(["-dmenu", "-p", "Clipboard", "-show-icons", "-sep", "\n", "-i"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute rofi");

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(items.join("\n").as_bytes());
    }

    if let Ok(output) = child.wait_with_output() {
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() { return; }

        if let (Some(start), Some(end)) = (selected.find('['), selected.find(']')) {
            let db_id = selected[start+1..end].trim();
            let _ = Command::new(BIN)
                .args(["copy-to", "--id", db_id])
                .status();
        }
    }
}
