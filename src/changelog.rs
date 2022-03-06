﻿use crate::Commit;
use git2::Repository;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::{collections::HashMap, env, ops::Index};

pub struct Changelogs {
    repo: Repository,
    author_github_map: HashMap<String, String>,
    client: Client,
    github_html_url: String,
}

#[derive(Debug)]
pub struct MARKDOWN {
    pub package: String,
    pub content: String,
}

#[derive(Deserialize)]
struct GithubUser {
    login: String,
}

#[derive(Deserialize)]
struct GithubRepo {
    html_url: String,
}

impl Changelogs {
    pub fn get_md_message(&mut self, commit: &Commit) -> String {
        let message = commit.message().split("\n").nth(0).unwrap().trim();

        let author = commit.author().as_ref().unwrap();
        let md_hash = commit.hash().trim();
        let short_md_hash = &md_hash[0..7];

        let github_user_id = self.get_github_user_id(author);

        let re = Regex::new(r"\(#[0-9]*\)").unwrap();

        if re.is_match(message) {
            let pr_id = re
                .captures(message)
                .unwrap()
                .index(0)
                .replace("(", "")
                .replace(")", "");

            let pr_url = format!(
                "{github_url}/pull/{pr_id}",
                github_url = self.github_html_url,
                pr_id = pr_id
            );

            let md_message = format!(
                    "{message}. [{pr_id}]({pr_url}) [@{github_user_id}](https://github.com/{github_user_id})",
                    pr_id = pr_id,
                    message = message,
                    pr_url = pr_url,
                    github_user_id = github_user_id,
                );

            return md_message;
        }

        let commit_or_pr_url = format!(
            "{github_url}/commit/{short_md_hash}",
            github_url = self.github_html_url,
            short_md_hash = short_md_hash
        );

        let md_message = format!(
            "{message}. [{short_md_hash}]({commit_or_pr_url}) [@{github_user_id}](https://github.com/{github_user_id})",
            short_md_hash = short_md_hash,
            message = message,
            commit_or_pr_url = commit_or_pr_url,
            github_user_id = github_user_id,
        );

        md_message
    }
    pub fn gen_change_log_by_commit_list(
        &mut self,
        commit_list: Vec<Commit>,
        package: &str,
    ) -> crate::Result<Vec<String>> {
        let mut changelog_list: Vec<String> = vec![];

        let mut commit_hash_map: HashMap<String, bool> = HashMap::new();

        for commit in commit_list {
            let message = commit.message().split("\n").nth(0).unwrap();
            let hash = commit.hash().to_string();

            let re = Regex::new(r"[fix|feat]\(([0-9a-zA-Z_]*)\)").unwrap();

            let mut need_insert_message = false;

            if re.is_match(message) {
                if re
                    .captures(message)
                    .unwrap()
                    .index(1)
                    .to_lowercase()
                    .eq(package)
                {
                    need_insert_message = true
                }
            }

            if need_insert_message && !commit_hash_map.get(&hash).is_some() {
                let md_message = self.get_md_message(&commit);
                changelog_list.insert(changelog_list.len(), md_message.clone());

                commit_hash_map.insert(hash, true);
            }
        }

        Ok(changelog_list)
    }

    pub fn gen_change_log_to_md(&mut self, tag_name: &str, change_logs: Vec<String>) -> String {
        let mut md_file_content: String = "".to_owned();

        md_file_content.push_str(&("## ".to_owned() + tag_name + "\n\n"));

        for changelog in change_logs {
            // 格式化成这个样子
            //  * feat(layout): mix support headerContent render [@chenshuai2144](https://github.com/chenshuai2144)
            md_file_content.push_str(&("* ".to_owned() + &changelog + "\n"));
        }

        md_file_content
    }
    // 获取所有包的change log，会循环一下
    pub fn get_change_log_list(&mut self) -> Vec<MARKDOWN> {
        let mut md_packages: Vec<MARKDOWN> = vec![];
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
            let (tag, commit_list) =
                crate::git::latest_commits(&self.repo, &("@ant-design/pro-".to_owned() + package))
                    .unwrap();

            let change_logs = self
                .gen_change_log_by_commit_list(commit_list, package)
                .unwrap();

            if change_logs.len() < 1 {
                // 如果数量不够就直接退出
                continue;
            }

            let md_file_content =
                self.gen_change_log_to_md(tag.name().as_ref().unwrap(), change_logs);

            md_packages.insert(
                md_packages.len(),
                MARKDOWN {
                    package: package.to_owned(),
                    content: md_file_content,
                },
            );
        }

        md_packages
    }

    /**
     * 获取所有的changelog
     * 会遍历所有的标签
     */
    pub fn get_all_change_log_list(&mut self) -> Vec<MARKDOWN> {
        let mut md_packages: Vec<MARKDOWN> = vec![];
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
            let mut package_md: Vec<String> = vec![];
            let commit_and_tag_list =
                crate::git::full_commits(&self.repo, &("@ant-design/pro-".to_owned() + package))
                    .unwrap();

            for commit_and_tag in commit_and_tag_list {
                let change_logs = self
                    .gen_change_log_by_commit_list(commit_and_tag.commit_list, package)
                    .unwrap();

                if change_logs.len() < 1 {
                    // 如果数量不够就直接退出
                    continue;
                }

                let md_file_content = self
                    .gen_change_log_to_md(commit_and_tag.tag.name().as_ref().unwrap(), change_logs);

                package_md.insert(package_md.len(), md_file_content);
            }

            md_packages.insert(
                md_packages.len(),
                MARKDOWN {
                    package: package.to_owned(),
                    content: package_md.join("\n\n"),
                },
            )
        }
        md_packages
    }

    // 根据github的开放的 API 获取用户id
    // 陈帅 -> chenshuai2144
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

    /**
     * 初始化，需要添加项目的地址
     */
    pub fn new(repo: String) -> Changelogs {
        let author_github_map = HashMap::new();
        let client = Client::new();
        let repo = Repository::open(repo).unwrap();

        //  仓库的 http 地址，用于生成 commit 的链接
        let repo_name = repo
            .find_remote("origin")
            .unwrap()
            .url()
            .unwrap()
            // git@github.com:ant-design/pro-components.git
            // -> ant-design/pro-components.git
            .split(":")
            .nth(1)
            .unwrap()
            .split(".")
            // ant-design/pro-components.git -> ant-design/pro-components
            .nth(0)
            .unwrap()
            .to_owned();

        let url = format!(
            "https://api.github.com/repos/{repo_name}",
            repo_name = repo_name
        );

        let body: GithubRepo = client
            .get(&url)
            .header(
                "Authorization",
                "token ".to_owned() + &env::var("GITHUB_TOKEN").unwrap(),
            )
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .unwrap()
            .json()
            .unwrap();

        let html_url = body.html_url;

        Changelogs {
            repo,
            client: client,
            author_github_map: author_github_map,
            github_html_url: html_url,
        }
    }
}
