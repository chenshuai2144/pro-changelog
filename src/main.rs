mod changelog;
mod error;
mod git;

use std::fs::{create_dir, File};
use std::io::Write;

use changelog::Changelogs;

pub use crate::error::{Error, ErrorKind, Result};
pub use crate::git::{latest_diff, Commit, Tag};

fn create_md_file(package: String, content: String) {
    let dir_path = ".changelog";

    let path = format!(".changelog/{package}.md", package = package);

    if !std::path::Path::new(&dir_path).exists() {
        create_dir(dir_path).unwrap();
    }

    let mut buffer = File::create(path).unwrap();

    buffer.write_all(content.as_bytes()).unwrap();
    buffer.flush().unwrap();
}

fn main() {
    // let md_file_content_list =
    //     Changelogs::new("C:/github/pro-components".to_string()).get_change_log_list();

    let md_file_content_list =
        Changelogs::new("C:/github/pro-components".to_string()).get_all_change_log();

    for md_file_content in md_file_content_list {
        create_md_file(md_file_content.package, md_file_content.content);
    }
}
