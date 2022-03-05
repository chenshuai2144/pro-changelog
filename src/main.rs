mod changelog;
mod error;
mod git;

use std::env;

use changelog::Changelogs;

pub use crate::error::{Error, ErrorKind, Result};
pub use crate::git::{all_commits, full_diff, Commit, Tag};

fn main() {
    Changelogs::new("C:/github/pro-components".to_string()).get_change_logs();
    //     Changelogs::new("/Users/shuaichen/Documents/github/pro-components".to_string())
    //         .get_change_logs();
}
