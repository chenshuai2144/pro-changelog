use std::{collections::HashMap, env};

use reqwest::Client;
use serde::Deserialize;

use crate::all_commits;

pub struct Changelogs {
    repo: String,
    author_github_map: HashMap<String, String>,
    client: Client,
}

#[derive(Deserialize)]
struct GithubUser {
    login: String,
}

impl Changelogs {
    pub fn new(repo: String) -> Changelogs {
        let author_github_map = HashMap::new();
        let client = Client::new();
        Changelogs {
            repo: repo,
            client: client,
            author_github_map: author_github_map,
        }
    }

    pub fn get_github_user_id(&mut self, author: &str) -> String {
        if self.author_github_map.get(author).is_none() {
            let mut map = HashMap::new();

            map.insert("name", author);
            let commit_url = "https://api.github.com/user";

            // 从 github 获取他叫啥
            let body: GithubUser = self
                .client
                .get(commit_url)
                .header(
                    "Authorization",
                    "token ".to_owned() + &env::var("GITHUB_TOKEN").unwrap(),
                )
                .header("Accept", "application/vnd.github.v3+json")
                .json(&map)
                .send()
                .unwrap()
                .json()
                .unwrap();

            // 存到一个map里面，防止多次请求
            self.author_github_map
                .insert(author.to_string(), body.login);
        }

        // 返回 map 里面对于 name 的映射
        self.author_github_map.get(author).unwrap().to_string()
    }
    pub fn get_change_log(&mut self, package: &str) -> Vec<String> {
        let mut changelog_list: Vec<String> = vec![];

        let (tag, commit_list) =
            all_commits(&self.repo, &("@ant-design/pro-".to_owned() + package)).unwrap();

        for commit in commit_list.clone() {
            let message = commit.message();
            let author = commit.author().as_ref().unwrap();

            let fix_message_start = format!("fix({package})", package = package);
            let feat_message_start = format!("feat({package})", package = package);
            if message.starts_with(&fix_message_start) || message.starts_with(&feat_message_start) {
                let github_user_id = self.get_github_user_id(author);
                let md_message = format!(
                    "{message} [@{github_user_id}](https://github.com/{github_user_id})",
                    message = message.split("\n").nth(0).unwrap(),
                    github_user_id = github_user_id,
                );
                changelog_list.insert(changelog_list.len(), md_message.clone());
            }
        }

        changelog_list
    }

    pub fn get_change_logs(&mut self) -> Vec<String> {
        let mut md_packages: Vec<String> = vec![];
        let package_list = [
            "utils",
            "layout",
            "form",
            "table",
            "field",
            "card",
            "descriptions",
        ];

        for package in package_list {
            let md: Vec<String> = self.get_change_log(package);
            println!("{:?}", md.join("\n"));
            md_packages.insert(md_packages.len(), md.join("\n"));
        }

        md_packages
    }
}
