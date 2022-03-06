use chrono::prelude::*;
use failure::ResultExt;
use git2::{self, DiffStatsFormat, Repository};
use semver::Version;
use std::{fmt::Result, str};

#[derive(Clone, Debug)]
pub struct TagAndVersion {
    package: String,
    version: String,
}

/// A git tag.
#[derive(Clone, Debug)]
pub struct Tag {
    name: Option<String>,
}

impl Tag {
    /// Access the tag name.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &Option<String> {
        &self.name
    }
}

/// A commit range for a tagged release
#[derive(Clone, Debug)]
pub struct CommitRange<'r> {
    latest_tag: Tag,
    start: git2::Commit<'r>,
    end: git2::Commit<'r>,
}

/// A git commit.
#[derive(Clone, Debug)]
pub struct Commit {
    message: String,
    hash: String,
    author: Option<String>,
    datetime: DateTime<Utc>,
}

impl Commit {
    /// Access the commit message.
    #[inline]
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Access the commit hash.
    #[inline]
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Access the commit author.
    #[inline]
    #[must_use]
    pub fn author(&self) -> &Option<String> {
        &self.author
    }

    /// Access the commit datetime.
    #[inline]
    #[must_use]
    pub fn datetime(&self) -> &DateTime<Utc> {
        &self.datetime
    }
}

/// Diff two git objects.
pub fn diff(repo: &Repository, o1: git2::Commit, o2: git2::Commit) -> crate::Result<String> {
    let t1 = o1.tree().context(crate::ErrorKind::Git)?;
    let tree2 = o2.tree().context(crate::ErrorKind::Git)?;
    // If o2 is the first object then we want to include it in the diff
    // so we diff o1 with None
    let t2 = match o2.parent(0) {
        Err(_err) => None,
        Ok(_parent) => Some(&tree2),
    };
    let diff = repo
        .diff_tree_to_tree(t2, Some(&t1), None)
        .context(crate::ErrorKind::Git)?;
    let stats = diff.stats().context(crate::ErrorKind::Git)?;
    let format = DiffStatsFormat::FULL;
    let buf = stats.to_buf(format, 80).context(crate::ErrorKind::Git)?;
    let buf = str::from_utf8(&*buf).context(crate::ErrorKind::Other)?;
    Ok(buf.to_owned())
}

/**
 * 获取 tag 和 version
 */
fn get_version(tag: &str) -> TagAndVersion {
    let package_list = tag.split('@').collect::<Vec<&str>>();
    let tag = TagAndVersion {
        package: "@".to_owned() + &package_list.get(1).unwrap().to_string(),
        version: package_list.last().unwrap().to_string(),
    };
    tag
}

/**
 * 排序 Tag，根据tag中带的版本号
 */
fn sort_tags<'a>(tags: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut tags = tags
        .filter_map(|tag| {
            Version::parse(&get_version(tag).to_owned().version)
                .ok()
                .map(|version| (tag, version))
        })
        .collect::<Vec<_>>();

    tags.sort_by(|(_, a), (_, b)| a.cmp(b));

    tags.into_iter().map(|(tag, _)| tag).collect()
}

/// 获取commit 的范围，默认获取的是 latest
pub fn get_commit_latest_range<'r>(
    repo: &'r Repository,
    package_name: &str,
) -> crate::Result<CommitRange<'r>> {
    let tag_list = repo.tag_names(None).context(crate::ErrorKind::Git)?;

    let tags = sort_tags(
        tag_list
            .into_iter()
            .filter_map(|x| x)
            .filter(|x| x.starts_with(package_name)),
    );
    let len = tags.len();

    let (start, end) = match len {
        0 => return Err(crate::ErrorKind::NoTags.into()),
        1 => (tags.get(len - 1), None),
        _ => (tags.get(len - 1), tags.get(len - 2)),
    };

    // Value has to be `Some()` here.
    let start_str = start.expect("Tag should have a value.");
    let (start, end) = match (start_str, end) {
        (start, None) => {
            let start = repo.revparse_single(start).context(crate::ErrorKind::Git)?;
            let mut reveals = repo.revwalk().context(crate::ErrorKind::Git)?;
            reveals.push(start.id()).context(crate::ErrorKind::Git)?;
            let oid = reveals
                .nth(0)
                .ok_or(crate::ErrorKind::Git)?
                .context(crate::ErrorKind::Git)?;
            let last = repo.find_object(oid, None).unwrap();
            (start, last)
        }
        (start, Some(end)) => (
            repo.revparse_single(start).context(crate::ErrorKind::Git)?,
            repo.revparse_single(end).context(crate::ErrorKind::Git)?,
        ),
    };

    let cr = CommitRange {
        start: start
            .peel_to_commit()
            .expect("There's no commit at the start point"),
        end: end
            .peel_to_commit()
            .expect("There's no commit at the end point"),
        latest_tag: Tag {
            name: Some((*start_str).to_owned()),
        },
    };

    Ok(cr)
}

/// Get the full diff in a single convenience function.
pub fn latest_diff(path: &str, package_name: &str) -> crate::Result<String> {
    let repo = Repository::open(path).context(crate::ErrorKind::Git)?;
    let commit_range = get_commit_latest_range(&repo, package_name)?;
    let start = commit_range.start;
    let end = commit_range.end;
    Ok(diff(&repo, start, end)?)
}
pub fn get_all_tag_range<'r>(
    repo: &'r Repository,
    package_name: &str,
) -> crate::Result<Vec<CommitRange<'r>>> {
    let mut cr_list: Vec<CommitRange> = vec![];
    let tag_list = repo.tag_names(None).context(crate::ErrorKind::Git)?;

    let tags = sort_tags(
        tag_list
            .into_iter()
            .filter_map(|x| x)
            .filter(|x| x.starts_with(package_name)),
    );

    let len = tags.len();

    let mut index = 0;

    while cr_list.len() < len - 1 {
        let (start, end) = match len {
            0 => return Err(crate::ErrorKind::NoTags.into()),
            1 => (tags.get(len - index - 1), None),
            _ => (tags.get(len - index - 1), tags.get(len - index - 2)),
        };

        // Value has to be `Some()` here.
        let start_str = start.expect("Tag should have a value.");
        let (start, end) = match (start_str, end) {
            (start, None) => {
                let start = repo.revparse_single(start).context(crate::ErrorKind::Git)?;
                let mut reveals = repo.revwalk().context(crate::ErrorKind::Git)?;
                reveals.push(start.id()).context(crate::ErrorKind::Git)?;
                let oid = reveals
                    .nth(0)
                    .ok_or(crate::ErrorKind::Git)?
                    .context(crate::ErrorKind::Git)?;
                let last = repo.find_object(oid, None).unwrap();
                (start, last)
            }
            (start, Some(end)) => (
                repo.revparse_single(start).context(crate::ErrorKind::Git)?,
                repo.revparse_single(end).context(crate::ErrorKind::Git)?,
            ),
        };

        let cr = CommitRange {
            start: start
                .peel_to_commit()
                .expect("There's no commit at the start point"),
            end: end
                .peel_to_commit()
                .expect("There's no commit at the end point"),
            latest_tag: Tag {
                name: Some((*start_str).to_owned()),
            },
        };
        cr_list.insert(cr_list.len(), cr);
        index = index + 1;
    }

    Ok(cr_list)
}

pub fn full_diff(path: &str, package_name: &str) -> crate::Result<Vec<String>> {
    let repo = Repository::open(path).context(crate::ErrorKind::Git)?;
    let commit_range_list = get_all_tag_range(&repo, package_name)?;
    let mut diff_list: Vec<String> = vec![];
    let mut index = 0;
    for commit_range in commit_range_list {
        let start = commit_range.start;
        let end = commit_range.end;
        let diff_str = diff(&repo, start, end)?;
        diff_list.insert(index, diff_str);
        index = index + 1;
    }

    Ok(diff_list)
}
/// Get all commits for a path.
pub fn latest_commits(path: &str, package_name: &str) -> crate::Result<(Tag, Vec<Commit>)> {
    let repo = Repository::open(path).context(crate::ErrorKind::Git)?;

    let commit_range = get_commit_latest_range(&repo, package_name)?;

    let tag = commit_range.latest_tag;
    let start = commit_range.start;
    let end = commit_range.end;

    let end_is_first_commit = match end.parent(0) {
        Err(_err) => true,
        _ => false,
    };

    let mut revwalk = repo.revwalk().context(crate::ErrorKind::Git)?;
    revwalk.push(start.id()).context(crate::ErrorKind::Git)?;
    let revwalk = revwalk.filter_map(|id| repo.find_commit(id.ok()?).ok());

    let mut commits = vec![];
    for commit in revwalk {
        if end.id() == commit.id() && !end_is_first_commit {
            break;
        }
        let message = commit.message().ok_or(crate::ErrorKind::Git)?.to_string();

        let hash = format!("{}", commit.id());
        let author = commit.author().name().map(|name| name.to_owned());
        let timestamp = commit.time().seconds();
        let naive_datetime = NaiveDateTime::from_timestamp(timestamp, 0);
        let datetime: DateTime<Utc> = DateTime::from_utc(naive_datetime, Utc);
        commits.push(Commit {
            message,
            hash,
            author,
            datetime,
        });
    }

    Ok((tag, commits))
}

pub struct TagAndCommit {
    pub tag: Tag,
    pub commit_list: Vec<Commit>,
}

pub fn full_commits(path: &str, package_name: &str) -> crate::Result<Vec<TagAndCommit>> {
    let repo = Repository::open(path).context(crate::ErrorKind::Git)?;
    let commit_range_list = get_all_tag_range(&repo, package_name)?;
    let mut commit_list: Vec<TagAndCommit> = vec![];
    let mut index = 0;
    for commit_range in commit_range_list {
        let tag = commit_range.latest_tag;
        let start = commit_range.start;
        let end = commit_range.end;

        let end_is_first_commit = match end.parent(0) {
            Err(_err) => true,
            _ => false,
        };

        let mut revwalk = repo.revwalk().context(crate::ErrorKind::Git)?;
        revwalk.push(start.id()).context(crate::ErrorKind::Git)?;
        let revwalk = revwalk.filter_map(|id| repo.find_commit(id.ok()?).ok());

        let mut commits = vec![];
        for commit in revwalk {
            if end.id() == commit.id() && !end_is_first_commit {
                break;
            }
            let message = commit.message().ok_or(crate::ErrorKind::Git)?.to_string();

            let hash = format!("{}", commit.id());
            let author = commit.author().name().map(|name| name.to_owned());
            let timestamp = commit.time().seconds();
            let naive_datetime = NaiveDateTime::from_timestamp(timestamp, 0);
            let datetime: DateTime<Utc> = DateTime::from_utc(naive_datetime, Utc);
            commits.push(Commit {
                message,
                hash,
                author,
                datetime,
            });
        }

        commit_list.insert(
            index,
            TagAndCommit {
                tag,
                commit_list: commits,
            },
        );
        index = index + 1;
    }

    Ok(commit_list)
}
